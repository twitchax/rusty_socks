use criterion::{Criterion, black_box, criterion_group, criterion_main};

use rsocks::handshake::Handshake;
use rsocks::request::Request;

fn bench_handshake(c: &mut Criterion) {
    // VER=5, NMETHODS=1, METHODS=[NO AUTH].
    let data = [0x05u8, 0x01, 0x00];

    c.bench_function("handshake_from_data", |b| b.iter(|| Handshake::from_data(black_box(&data))));
}

fn bench_request_ipv4(c: &mut Criterion) {
    // VER=5, CMD=CONNECT, RSV=0, ATYP=IPv4, 127.0.0.1:80.
    let data = [0x05u8, 0x01, 0x00, 0x01, 127, 0, 0, 1, 0x00, 0x50];

    c.bench_function("request_from_data_ipv4", |b| b.iter(|| Request::from_data(black_box(&data)).unwrap()));
}

criterion_group!(benches, bench_handshake, bench_request_ipv4);
criterion_main!(benches);
