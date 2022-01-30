project(
    'rs_filecopy',
    version: '0.1'
)

cargo = find_program('cargo')

rs_filecopy = custom_target(
    'rs_filecopy',
    output: '.',
    command: [
        cargo,
        'build',
        '--profile', 'release-lto',
        '--target-dir', '@OUTPUT@',
        '--manifest-path', '../subprojects/rs_filecopy/Cargo.toml'
    ],
    build_by_default: true,
    install: false
)
