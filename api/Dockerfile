# =====================================================================
# Absensi Face Recognition API
#
# Build dua tahap. Tahap pertama meng-cache dependensi secara terpisah dari
# kode aplikasi, sehingga perubahan pada src/ tidak memaksa kompilasi ulang
# ~500 crate — perbedaannya sekitar 8 menit vs 40 detik per build.
# =====================================================================

# Versi disematkan agar build bulan depan menghasilkan biner yang sama.
#
# Batas bawahnya bukan pilihan bebas: beberapa dependensi di Cargo.lock
# (image, time, validator_derive) menuntut rustc >= 1.88. Menurunkannya
# membuat build gagal di tahap dependensi dengan "requires rustc 1.88.0",
# bukan di kode kita.
ARG RUST_VERSION=1.90
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        ca-certificates \
        # Build script utoipa-swagger-ui MENGUNDUH bundel Swagger UI saat
        # kompilasi dan memanggil `curl` untuk itu. Tanpa ini build gagal
        # dengan "failed to download Swagger UI: `curl` command not found" —
        # pesan yang menyesatkan, karena terlihat seperti masalah jaringan
        # padahal perkakasnya yang tidak ada.
        curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# 1) Hanya manifest -> layer dependensi yang stabil.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

# 2) Kode aplikasi yang sebenarnya.
COPY src ./src
COPY migrations ./migrations

# Sentuh main.rs agar cargo tahu binary harus dibangun ulang meski
# timestamp-nya lebih lama dari artefak dummy di atas.
RUN touch src/main.rs && cargo build --release --locked

# ---------------------------------------------------------------------
# Runtime: image ramping tanpa toolchain Rust.
# ---------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/* \
    # Proses tidak berjalan sebagai root: bila terjadi RCE, penyerang tidak
    # langsung memegang container.
    && useradd --system --create-home --shell /usr/sbin/nologin absensi

WORKDIR /app

COPY --from=builder /build/target/release/jargon-api /usr/local/bin/jargon-api
# Migrasi disertakan karena dijalankan saat start (sqlx memakai advisory lock,
# jadi aman walau beberapa replika start bersamaan).
COPY --from=builder /build/migrations /app/migrations

# Foto pendaftaran wajah. Di produksi WAJIB di-mount sebagai volume bersama;
# bila tidak, replika yang berbeda tidak akan menemukan berkas satu sama lain.
RUN mkdir -p /app/storage && chown -R absensi:absensi /app

USER absensi

ENV BIND_ADDR=0.0.0.0:8080 \
    STORAGE_ROOT=/app/storage \
    RUST_LOG=jargon_api=info,tower_http=info,sqlx=warn

EXPOSE 8080

# Liveness memakai /health/live yang sengaja TIDAK menyentuh database:
# database yang lambat tidak boleh membuat orchestrator me-restart pod dan
# memutus absensi yang sedang berjalan.
HEALTHCHECK --interval=15s --timeout=3s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health/live || exit 1

ENTRYPOINT ["/usr/local/bin/jargon-api"]
