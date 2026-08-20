<?php

return [

    /*
    |--------------------------------------------------------------------------
    | Third Party Services
    |--------------------------------------------------------------------------
    |
    | This file is for storing the credentials for third party services such
    | as Mailgun, Postmark, AWS and more. This file provides the de facto
    | location for this type of information, allowing packages to have
    | a conventional file to locate the various service credentials.
    |
    */

    'postmark' => [
        'key' => env('POSTMARK_API_KEY'),
    ],

    'resend' => [
        'key' => env('RESEND_API_KEY'),
    ],

    'ses' => [
        'key' => env('AWS_ACCESS_KEY_ID'),
        'secret' => env('AWS_SECRET_ACCESS_KEY'),
        'region' => env('AWS_DEFAULT_REGION', 'us-east-1'),
    ],

    'slack' => [
        'notifications' => [
            'bot_user_oauth_token' => env('SLACK_BOT_USER_OAUTH_TOKEN'),
            'channel' => env('SLACK_BOT_USER_DEFAULT_CHANNEL'),
        ],
    ],

    'midtrans' => [
        'merchant_id' => env('MIDTRANS_MERCHANT_ID'),
        'client_key' => env('MIDTRANS_CLIENT_KEY'),
        'server_key' => env('MIDTRANS_SERVER_KEY'),
        'is_production' => env('MIDTRANS_IS_PRODUCTION', false),
    ],

    'reverb' => [
        'app_id' => env('REVERB_APP_ID'),
        'key' => env('REVERB_APP_KEY'),
        'secret' => env('REVERB_APP_SECRET'),
        'host' => env('REVERB_HOST'),
        'port' => env('REVERB_PORT', 8080),
        'scheme' => env('REVERB_SCHEME', 'https'),
    ],

    /*
    |--------------------------------------------------------------------------
    | API Absensi Face Recognition (layanan Rust)
    |--------------------------------------------------------------------------
    |
    | `url` dipakai dashboard untuk memanggil API dari sisi server (boleh
    | alamat internal container). `public_url` dipakai untuk membangun tautan
    | yang dibuka BROWSER pengguna — mis. foto pendaftaran wajah di /files/*.
    |
    | `embedding_dim` dan `face_model_version` HARUS sama dengan nilai pada
    | API dan pada model TFLite/TensorFlow.js di perangkat. Embedding dari
    | versi model berbeda tidak sebanding, dan mencocokkannya menghasilkan
    | identifikasi acak — karena itu server memvalidasinya pada setiap request.
    |
    */
    'absensi_api' => [
        'url' => env('ABSENSI_API_URL', 'http://127.0.0.1:8080'),
        'public_url' => env('ABSENSI_API_PUBLIC_URL', env('ABSENSI_API_URL', 'http://127.0.0.1:8080')),
        'key_id' => env('ABSENSI_API_KEY_ID'),
        'secret' => env('ABSENSI_API_SECRET'),
        'timeout' => env('ABSENSI_API_TIMEOUT', 20),
        'embedding_dim' => env('FACE_EMBEDDING_DIM', 512),
        'face_model_version' => env('FACE_MODEL_VERSION', 'mobilefacenet-v1'),
        'docs_url' => env('ABSENSI_API_DOCS_URL', env('ABSENSI_API_PUBLIC_URL', 'http://127.0.0.1:8080').'/docs'),
    ],

];
