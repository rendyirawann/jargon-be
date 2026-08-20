<?php

namespace App\Http\Controllers\Backend\Device;

use App\Http\Controllers\Controller;
use App\Models\Classroom;
use App\Models\Device;
use App\Services\AbsensiApi;
use App\Support\Tenant;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Routing\Controllers\HasMiddleware;
use Illuminate\Routing\Controllers\Middleware;
use Illuminate\View\View;

/**
 * Tablet kios di sekolah.
 *
 * Pembuatan perangkat dan kode pairing dilakukan lewat API Rust — token dan
 * kunci HMAC harus dihasilkan di sana agar dashboard tidak pernah menyimpan
 * atau bahkan melihat bentuk mentahnya lebih lama dari satu tampilan.
 */
class DeviceController extends Controller implements HasMiddleware
{
    public static function middleware(): array
    {
        return [
            'auth',
            new Middleware('can:view_device', only: ['index', 'show']),
            new Middleware('can:create_device', only: ['store']),
            new Middleware('can:update_device', only: ['update', 'revoke']),
            new Middleware('can:pair_device', only: ['pairingCode']),
            new Middleware('can:delete_device', only: ['destroy']),
        ];
    }

    public function index(Request $request): View
    {
        $schoolId = Tenant::currentSchoolId($request->query('school_id'));

        $devices = Device::query()
            ->when($schoolId, fn ($q) => $q->where('school_id', $schoolId))
            ->with(['school:id,name', 'classroom:id,name'])
            ->orderBy('code')
            ->get();

        return view('backend.device.index', [
            'devices' => $devices,
            'schoolId' => $schoolId,
            'schools' => Tenant::isProvinceScope() ? Tenant::selectableSchools() : collect(),
            'classrooms' => $this->classroomOptions($schoolId),
            'placements' => Device::PLACEMENTS,
            'modes' => Device::MODES,
            'stats' => [
                'total' => $devices->count(),
                'online' => $devices->where('is_online', true)->count(),
                'unpaired' => $devices->where('is_paired', false)->count(),
            ],
        ]);
    }

    public function store(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'school_id' => ['required', 'exists:schools,id'],
            'code' => ['required', 'string', 'min:3', 'max:40', 'unique:devices,code'],
            'name' => ['required', 'string', 'min:3', 'max:120'],
            'placement' => ['required', 'in:'.implode(',', Device::PLACEMENTS)],
            'classroom_id' => ['nullable', 'exists:classrooms,id'],
            'mode' => ['required', 'in:'.implode(',', Device::MODES)],
        ], [], [
            'code' => 'kode perangkat',
            'name' => 'nama perangkat',
        ]);

        Tenant::authorizeSchool($data['school_id']);

        // Tablet di dalam kelas harus tahu kelasnya: daftar siswa yang
        // diunduh dibatasi ke kelas itu, dan arah scan mengikuti aturannya.
        if ($data['placement'] === 'classroom' && empty($data['classroom_id'])) {
            return back()->withInput()->withErrors([
                'classroom_id' => 'Perangkat di dalam kelas wajib menyebutkan kelasnya.',
            ]);
        }

        $result = AbsensiApi::make()->createDevice(AbsensiApi::tokenFromSession(), $data);

        if (! $result['success']) {
            return back()->withInput()->withErrors($result['errors'] ?: ['code' => $result['message']]);
        }

        $payload = $result['data'] ?? [];

        // Kode pairing hanya berguna sebentar, jadi ditampilkan sekali lewat
        // flash session dan tidak disimpan di mana pun.
        return back()
            ->with('success', $result['message'])
            ->with('pairing', [
                'device_code' => $payload['code'] ?? $data['code'],
                'pairing_code' => $payload['pairing_code'] ?? null,
                'expires_at' => $payload['expires_at'] ?? null,
            ]);
    }

    public function update(Request $request, Device $device): RedirectResponse
    {
        Tenant::authorizeSchool($device->school_id);

        $data = $request->validate([
            'name' => ['required', 'string', 'min:3', 'max:120'],
            'placement' => ['required', 'in:'.implode(',', Device::PLACEMENTS)],
            'classroom_id' => ['nullable', 'exists:classrooms,id'],
            'mode' => ['required', 'in:'.implode(',', Device::MODES)],
            'is_active' => ['nullable', 'boolean'],
        ]);

        if ($data['placement'] === 'classroom' && empty($data['classroom_id'])) {
            return back()->withErrors([
                'classroom_id' => 'Perangkat di dalam kelas wajib menyebutkan kelasnya.',
            ]);
        }

        $data['is_active'] = (bool) ($data['is_active'] ?? false);
        $device->update($data);

        return back()->with('success', "Perangkat {$device->code} diperbarui.");
    }

    public function pairingCode(Device $device): RedirectResponse
    {
        Tenant::authorizeSchool($device->school_id);

        $result = AbsensiApi::make()->regeneratePairingCode(
            AbsensiApi::tokenFromSession(),
            $device->id
        );

        if (! $result['success']) {
            return back()->withErrors(['device' => $result['message']]);
        }

        $payload = $result['data'] ?? [];

        return back()
            ->with('success', $result['message'])
            ->with('pairing', [
                'device_code' => $device->code,
                'pairing_code' => $payload['pairing_code'] ?? null,
                'expires_at' => $payload['expires_at'] ?? null,
            ]);
    }

    public function revoke(Device $device): RedirectResponse
    {
        Tenant::authorizeSchool($device->school_id);

        $result = AbsensiApi::make()->revokeDevice(AbsensiApi::tokenFromSession(), $device->id);

        return $result['success']
            ? back()->with('success', "Token {$device->code} dicabut. Tablet harus dipasangkan ulang.")
            : back()->withErrors(['device' => $result['message']]);
    }

    public function show(Device $device): View
    {
        Tenant::authorizeSchool($device->school_id);

        return view('backend.device.show', [
            'device' => $device->load('school:id,name', 'classroom:id,name'),
            'heartbeats' => \Illuminate\Support\Facades\DB::table('device_heartbeats')
                ->where('device_id', $device->id)
                ->orderByDesc('reported_at')
                ->limit(50)
                ->get(),
            // Statistik scan hari ini — bukti perangkat benar-benar dipakai,
            // bukan hanya online.
            'todayScans' => \Illuminate\Support\Facades\DB::table('attendance_events')
                ->where('device_id', $device->id)
                ->whereRaw("occurred_at >= CURRENT_DATE")
                ->selectRaw("
                    COUNT(*) AS total,
                    COUNT(*) FILTER (WHERE outcome = 'accepted') AS accepted,
                    COUNT(*) FILTER (WHERE outcome = 'rejected') AS rejected,
                    COUNT(*) FILTER (WHERE event_type = 'unknown') AS unknown,
                    ROUND(AVG(latency_ms)) AS avg_latency
                ")
                ->first(),
        ]);
    }

    public function destroy(Device $device): RedirectResponse
    {
        Tenant::authorizeSchool($device->school_id);

        $code = $device->code;
        $device->update([
            'is_active' => false,
            'token_revoked_at' => now(),
            'pairing_code' => null,
        ]);
        $device->delete();

        return back()->with('success', "Perangkat {$code} dihapus dan tokennya dicabut.");
    }

    private function classroomOptions(?string $schoolId)
    {
        if (! $schoolId) {
            return collect();
        }

        return Classroom::where('school_id', $schoolId)
            ->where('is_active', true)
            ->orderBy('grade_level')
            ->orderBy('name')
            ->get(['id', 'name']);
    }
}
