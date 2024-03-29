# todo

- 🟠 graphics
    - 🟠 basic shader
        - 🔘 essential transforms, colours, textures
        - 🟠 alpha testing
            - 🔴 selective
    - 🟠 flythrough camera
        - 🔘 debug/smooth out cr-spline
        - 🔴 jumps
        - 🔴 iron out consistent glitches on some tracks
    - 🔘 sprites
- 🟠 input
    - 🔘 debug keyboard controls
    - 🔴 pad support
- 🟠 assets
    - 🟠 track extraction
        - 🔴 bake scenery objects together
        - 🟠 visuals
            - 🔘 basic mesh
            - 🔘 base textures
            - 🔴 render boosts and pickups
            - 🔴 different rendering for backfaces
            - 🔴 texture patches (.tex; 2097/xl)
        - 🟠 sections etc
            - 🔘 form camera spline path
    - 🟠 scene/object extraction
        - 🔘 poly meshes
        - 🔴 one/two sided polys
        - 🔴 selective transparency
        - 🔘 sprites
    - 🟠 textures
        - 🔘 basic conversion
        - 🔘 qoi encoding
        - 🔴 correct alpha extraction
        - 🔘 atlases
            - 🔘 pack individual + build atlases at load
                - better compression
    - 🟠 asset bundling
        - 🔘 lz4 compression
        - 🟠 better storage
            - 🔘 uv
            - 🟠 rgb
            - 🔴 xyz
- 🔴 physics
- 🔴 ai
- 🟠 ui
    - 🔴 menus
    - 🟠 fonts
        - 🔘 choose fonts [^1]
                - general use: supErphonix2, fusion, x2
                - some use: 2097, wo3, amalgama
        - 🟠 baked into 3d meshes
            - 🔴 normals
        - 🟠 metrics + layout
            - 🔘 advance
            - 🔴 offset (lsb etc)
            - 🔴 line-line
            - 🔴 kerning
- 🟠 sound
    - 🟠 sfx extraction
        - 🔘 adpcm decompression
        - 🔴 parse `.vh` for correct rates

[^1]: rationale
    - accepted for general use
        - supErphonix: good; has case; good coverage (improved f5000)
        - fusion:      good; has case; good coverage (overall good; bad 'v')
        - x2:          fair; has case; good coverage
    - accepted for selective use
        - 2097:     fair;  no case; good coverage (improved amalgama)
        - wo3:      poor;  no case; good coverage
        - amalgama: awful; no case; poor coverage (the og; some titling only)
    - rejected
        - fx300 ang:  fair; no case; good coverage (but bad symbol shapes)
        - 2197 block: fair; no case; poor coverage
        - 2197 heavy: fair; no case; poor coverage (colony wars lmao)
        - fx300:      fair; no case; awful coverage
        - f5000:      poor; no case; fair coverage
        - f500 ang:   poor; no case; fair coverage (worse wo3)
        - assegai:    poor; no case; awful coverage

# bad texture that breaks rapid-qoi

track16, scene.cmp[8], 148x75

