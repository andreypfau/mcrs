use mcrs_nbt::compound::NbtCompound;

#[allow(dead_code)]
enum DialogHolder {
    Direct(NbtCompound),
    Reference(u32),
}
