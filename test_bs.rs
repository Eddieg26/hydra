use fixedbitset::FixedBitSet;
fn main() {
    let mut bs = FixedBitSet::new();
    bs.grow_and_insert(2);
    println!("contains(0): {}", bs.contains(0));
    println!("contains(1): {}", bs.contains(1));
    println!("contains(2): {}", bs.contains(2));
    println!("ones: {:?}", bs.ones().collect::<Vec<_>>());
}
