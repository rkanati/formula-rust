
# tracks - data may be big endian?!

## vertices: .trv
    bare array of
    ```rust
    [x, y, z, _]: [i32be; 4]
    ```

## faces:.trf
    bare array of
    ```rust
    struct VertIdx(i16be);
    struct Face {
        verts:  [VertIdx; 4], // 8
        normal: [i16be; 3],   // 14
        tex:    u8,           // 15
        flags:  u8,           // 16
        colour: [u8; 3],      // 19
        _pad:   u8            // 20?
    }
    ```

## sections: .trs
    base array of
    ```rust
    struct SectIdx(i16be);
    struct Section {
        junction: SectIdx,          //   4
        prev:     SectIdx,          //   8
        next:     SectIdx,          //  12
        center:   [i32be; 3],       //  24
        version:  i16be,            //  26
        _pad0:    i16be,            //  28
        _objs:    Ptr,              //  32
        objs_n:   i16,              //  34
        _pad1:    [u8;2],           //  36
        _views:   [[Ptr; 3]; 5],    //  96
        view_ns:  [[i16be; 3]; 5],  // 126
        high:     [i16be; 4],       // 134
        med:      [i16be; 4],       // 142
        face_st:  i16be,            // 144
        face_n:   i16be,            // 146
        r_global: i16be,            // 148
        r_local:  i16be,            // 150
        flags:    i16be,            // 152
        sect_i:   i16be,            // 154
        _pad2:    [u8;2],           // 156
    }
    ```

## views: .vew
    bare array of
    ```rust
    i16be
    ```

# compressed texture (.cmp) files

```rust
n_tims: i32,
tim_sizes: [i32; n_tims],
compressed: [u8]
```

