# python/pyinstaller.spec
# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec file for building Starlette/Hypercorn app as standalone executable

a = Analysis(
    ['app.py'],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=[
        'starlette',
        'starlette.applications',
        'starlette.responses',
        'starlette.routing',
        'hypercorn',
        'hypercorn.asyncio',
        'hypercorn.config',
        'anyio',
        'anyio._backends',
        'anyio._backends._asyncio',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludedimports=[],
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=None)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='ai-engine',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
