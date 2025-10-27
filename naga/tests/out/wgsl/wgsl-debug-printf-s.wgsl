fn main_1_() {
    return;
}

@compute @workgroup_size(1, 1, 1) 
fn main() {
    main_1_();
    return;
}
