use atomic_capsule::http::HttpKeepAliveCapsule;

fn main() {
    println!("HttpKeepAliveCapsule size: {}", std::mem::size_of::<HttpKeepAliveCapsule>());
    println!("HttpKeepAliveCapsule align: {}", std::mem::align_of::<HttpKeepAliveCapsule>());
}
