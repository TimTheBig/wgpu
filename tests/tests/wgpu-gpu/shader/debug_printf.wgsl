const twelve = 12;

@compute @workgroup_size(8,8,1)
fn main() {
    debugPrintf("Hello world %d", 1);
    debugPrintf("12 == %i", twelve);
}
