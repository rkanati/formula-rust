#[derive(Clone, Copy, rkyv::Archive, rkyv::Serialize)]
#[archive_attr(repr(transparent), derive(Debug, Clone, Copy))]
pub struct UNorm16(pub u16);

#[allow(dead_code)]
static CHECK_UNORM16_COMPILE: [(); 2-std::mem::size_of::<ArchivedUNorm16>()] = [];

#[derive(Clone, Copy, rkyv::Archive, rkyv::Serialize)]
#[archive_attr(repr(transparent), derive(Debug, Clone, Copy))]
pub struct UNorm8(pub u8);

#[allow(dead_code)]
static CHECK_UNORM8_COMPILE: [(); 1-std::mem::size_of::<ArchivedUNorm8>()] = [];

impl UNorm16 { pub fn new(x: f32) -> Self { Self((x.fract() * 65535.).round() as u16) } }
impl UNorm8  { pub fn new(x: f32) -> Self { Self((x.fract() *   255.).round() as u8) } }

impl From<UNorm16> for f32 { fn from(UNorm16(y): UNorm16) -> Self { y as f32 / 65_535. } }
impl From<UNorm8>  for f32 { fn from(UNorm8(y):   UNorm8) -> Self { y as f32 /    255. } }
impl From<ArchivedUNorm16> for f32 { fn from(y: ArchivedUNorm16) -> Self { y.0 as f32 / 65_535. } }
impl From<ArchivedUNorm8>  for f32 { fn from(y:  ArchivedUNorm8) -> Self { y.0 as f32 /    255. } }

#[allow(non_camel_case_types)]
pub type un16 = UNorm16;

#[allow(non_camel_case_types)]
pub type un8 = UNorm8;

