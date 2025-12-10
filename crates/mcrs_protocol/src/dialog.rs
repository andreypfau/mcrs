use mcrs_nbt::compound::NbtCompound;

enum DialogHolder {
    Direct(NbtCompound),
    Reference(u32),
}
