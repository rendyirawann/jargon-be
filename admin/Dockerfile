# =====================================================================
# Dashboard /admin — Laravel 12
#
# Sebelumnya dashboard dijalankan dari image php:8.3-cli polos, dengan
# ekstensi dipasang lewat apt-get SETIAP KALI container start. Itu cukup
# untuk satu mesin pengembang, tetapi tidak bisa diserahkan ke orang lain:
# start-up memakan beberapa menit, butuh internet, dan hasilnya berbeda
# tiap kali karena versi paket apt bergerak.
#
# Di sini semuanya dipanggang ke dalam image.
# =====================================================================

# ---------------------------------------------------------------------
# base — PHP + seluruh ekstensi, dipakai bersama oleh vendor dan runtime.
#
# Composer memeriksa versi PHP dan ekstensi terhadap SELURUH pohon
# dependensi sebelum mengunduh apa pun. Karena itu ekstensi harus sudah
# ada sejak tahap vendor, bukan hanya saat runtime — kalau tidak,
# `composer install` berhenti dengan "your php version does not satisfy".
#
# Versi PHP dipatok 8.3, bukan `latest`: composer.json meminta ^8.2 dan
# sebagian dependensi (openspout, lewat yajra/laravel-datatables-export)
# menolak PHP di atas 8.4. Memakai image composer resmi tidak bisa —
# image itu kini membawa PHP 8.5.
# ---------------------------------------------------------------------
FROM php:8.3-cli-bookworm AS base

RUN apt-get update && apt-get install -y --no-install-recommends \
        libpq-dev \
        libzip-dev \
        libicu-dev \
        libpng-dev \
        libjpeg62-turbo-dev \
        libfreetype6-dev \
        unzip \
        git \
        curl \
    && docker-php-ext-configure gd --with-freetype --with-jpeg \
    && docker-php-ext-install -j"$(nproc)" \
        pdo_pgsql \
        pgsql \
        zip \
        intl \
        gd \
        sockets \
        opcache \
    && pecl install redis \
    && docker-php-ext-enable redis \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------
# vendor — dependensi PHP
# ---------------------------------------------------------------------
FROM base AS vendor

# PHAR composer diambil dari image resminya, tetapi dijalankan dengan PHP
# 8.3 milik base. Yang menentukan pemeriksaan platform adalah PHP yang
# menjalankan composer, bukan versi composer itu sendiri.
COPY --from=composer:2 /usr/bin/composer /usr/bin/composer

WORKDIR /app

# Manifest lebih dulu -> layer vendor yang stabil. Perubahan pada app/
# tidak memaksa resolusi ulang seluruh dependensi.
COPY composer.json composer.lock ./

# --no-scripts: skrip artisan butuh kode aplikasi yang belum disalin.
# Autoload dibangun setelah kodenya lengkap, di bawah.
RUN composer install \
        --no-dev \
        --no-scripts \
        --no-autoloader \
        --prefer-dist \
        --no-interaction

COPY . .

RUN composer dump-autoload --optimize --no-dev --no-interaction

# ---------------------------------------------------------------------
# runtime
# ---------------------------------------------------------------------
FROM base

# OPcache menyala: dashboard adalah proses yang berumur panjang, dan
# mem-parsing ulang ribuan berkas PHP pada setiap request adalah biaya
# terbesar yang paling mudah dihilangkan.
RUN { \
        echo 'opcache.enable=1'; \
        echo 'opcache.memory_consumption=192'; \
        echo 'opcache.max_accelerated_files=20000'; \
        echo 'opcache.validate_timestamps=0'; \
        echo 'memory_limit=512M'; \
        echo 'upload_max_filesize=16M'; \
        echo 'post_max_size=20M'; \
    } > /usr/local/etc/php/conf.d/jargon.ini

WORKDIR /var/www/admin

COPY --from=vendor /app /var/www/admin

# Proses tidak berjalan sebagai root: bila terjadi RCE lewat unggahan
# berkas, penyerang tidak langsung memegang container.
#
# Direktori ditulis satu per satu, bukan dengan brace expansion
# `{cache,sessions,views}`: RUN dijalankan lewat /bin/sh yang di Debian
# adalah dash, dan dash tidak mengenal brace expansion — yang terbentuk
# justru satu direktori bernama harfiah "{cache,sessions,views}".
RUN useradd --system --create-home --shell /usr/sbin/nologin jargon \
    && mkdir -p \
        storage/framework/cache/data \
        storage/framework/sessions \
        storage/framework/views \
        storage/app/public \
        storage/logs \
        bootstrap/cache \
    && chown -R jargon:jargon /var/www/admin

COPY --chown=jargon:jargon docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

USER jargon

# Server bawaan PHP memproses SATU permintaan pada satu waktu. Satu halaman
# dashboard menarik puluhan aset dan beberapa panggilan AJAX; dengan satu
# pekerja, semuanya mengantre dan halaman terasa menggantung — bahkan bisa
# saling mengunci ketika satu request menunggu request lain selesai.
#
# Ini tetap server pengembangan. Untuk beban sebenarnya, ganti perintah di
# entrypoint dengan Octane (lihat docs/DEPLOYMENT.md).
ENV PHP_CLI_SERVER_WORKERS=8

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8000/admin/login >/dev/null || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
