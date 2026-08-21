{{--
    Penghapusan absensi — dipakai halaman detail siswa dan daftar absensi.

    Ditaruh di satu berkas supaya kedua halaman tidak menyimpan dua salinan
    aturan yang bisa berbeda diam-diam: konfirmasi, token, penanganan galat,
    dan apa yang terjadi setelah berhasil.

    Tombolnya hanya dirender untuk yang berizin (@can di markup), dan server
    memeriksa ulang lewat middleware can:delete_attendance — tampilan bukan
    penjaga keamanan.
--}}
<script>
    (function () {
        const TOKEN = @json(csrf_token());

        // Pola URL dibentuk dari route(), bukan disusun tangan: dashboard ini
        // dilayani di bawah subjalur (/jargon-be/admin), jadi menempel
        // '/admin/attendances' begitu saja akan salah.
        const POLA = @json(route('attendances.destroy', ['attendance' => '__ID__']));

        function beritahu(ikon, judul, teks) {
            if (window.Swal) {
                Swal.fire({ icon: ikon, title: judul, text: teks || '', confirmButtonText: 'Tutup' });
                return;
            }
            window.alert(judul + (teks ? '\n' + teks : ''));
        }

        // Delegasi di tingkat dokumen: baris pada daftar absensi digambar ulang
        // oleh DataTables setiap kali halaman tabel berganti, jadi memasang
        // pendengar per tombol saat muat awal akan kehilangan baris berikutnya.
        document.addEventListener('click', async function (ev) {
            const btn = ev.target.closest('[data-hapus-absensi]');
            if (!btn) return;
            ev.preventDefault();

            const id = btn.getAttribute('data-id');
            const tanggal = btn.getAttribute('data-tanggal');
            const nama = btn.getAttribute('data-nama') || 'siswa ini';
            const label = btn.getAttribute('data-label') || tanggal;
            if (!id || !tanggal) return;

            const kalimat = 'Absensi ' + nama + ' tanggal ' + label
                + ' akan dihapus permanen. Rekap dan laporan yang memuatnya ikut berubah.';

            let lanjut;
            if (window.Swal) {
                const jawab = await Swal.fire({
                    icon: 'warning',
                    title: 'Hapus absensi ini?',
                    text: kalimat,
                    showCancelButton: true,
                    confirmButtonText: 'Ya, hapus',
                    cancelButtonText: 'Batal',
                    reverseButtons: true,
                    customClass: { confirmButton: 'btn btn-danger', cancelButton: 'btn btn-light' },
                    buttonsStyling: false,
                });
                lanjut = jawab.isConfirmed;
            } else {
                lanjut = window.confirm(kalimat);
            }
            if (!lanjut) return;

            btn.setAttribute('disabled', 'disabled');

            let res, data = {};
            try {
                res = await fetch(POLA.replace('__ID__', encodeURIComponent(id)), {
                    method: 'DELETE',
                    headers: {
                        'X-CSRF-TOKEN': TOKEN,
                        'Content-Type': 'application/json',
                        'Accept': 'application/json',
                        'X-Requested-With': 'XMLHttpRequest',
                    },
                    credentials: 'same-origin',
                    body: JSON.stringify({ attendance_date: tanggal }),
                });
                data = await res.json().catch(function () { return {}; });
            } catch (e) {
                btn.removeAttribute('disabled');
                beritahu('error', 'Gagal menghubungi server', 'Absensi belum dihapus. Coba lagi.');
                return;
            }

            if (!res.ok) {
                btn.removeAttribute('disabled');
                beritahu('error', 'Gagal menghapus',
                    data.message || ('Server menjawab kode ' + res.status + '. Absensi belum dihapus.'));
                return;
            }

            // Halaman dimuat ulang, bukan cuma barisnya yang dibuang: angka
            // rekap di halaman yang sama dihitung dari absensi yang baru saja
            // hilang, dan angka yang basi lebih menyesatkan daripada menunggu
            // satu muat ulang.
            const selesai = function () { window.location.reload(); };

            if (window.Swal) {
                Swal.fire({
                    icon: 'success',
                    title: 'Absensi dihapus',
                    text: data.message || '',
                    confirmButtonText: 'Tutup',
                    customClass: { confirmButton: 'btn btn-primary' },
                    buttonsStyling: false,
                }).then(selesai);
            } else {
                selesai();
            }
        });
    })();
</script>
