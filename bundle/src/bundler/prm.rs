
mod prim;

struct Object {
    name: [u8; 16],

    n_verts: u16,
    _pad0: u16,
    _verts: u32,

    n_norms: u16,
    _pad1: u16,
    _norms: u32,

    n_prims: u16,
    _pad2: u16,
    _prims: u32,

    _lib_obj: u32,
    _bsp_tree: u32,
    _skeleton: u32,

    extent: i32,
    flags: u16,

    _pad3: u16,
    _next: u32,
}

