use criterion::{black_box, criterion_group, criterion_main, Criterion};
use atomic_network_gateway::{
    NetworkGateway, OrderGateway, MarketDataGateway, MessageHeader, MessageType,
    GenerationCounter, MessageBuffer,
};

fn benchmark_order_processing(c: &mut Criterion) {
    let gateway = OrderGateway::new();

    c.bench_function("order_send", |b| {
        b.iter(|| {
            let _ = gateway.send_order(black_box(42), black_box(b"test order"));
        })
    });

    c.bench_function("order_ack", |b| {
        b.iter(|| {
            let _ = gateway.process_ack(black_box(1));
        })
    });
}

fn benchmark_market_data_processing(c: &mut Criterion) {
    let gateway = MarketDataGateway::new();

    // Create valid message
    let header = MessageHeader::new(MessageType::MarketData, 32, 42, 1);
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            &header as *const _ as *const u8,
            std::mem::size_of::<MessageHeader>(),
        )
    };

    c.bench_function("market_data_process", |b| {
        b.iter(|| {
            let _ = gateway.process_market_data(black_box(header_bytes));
        })
    });
}

fn benchmark_generation_counter(c: &mut Criterion) {
    let counter = GenerationCounter::new();

    c.bench_function("generation_next", |b| {
        b.iter(|| {
            black_box(counter.next());
        })
    });

    c.bench_function("generation_current", |b| {
        b.iter(|| {
            black_box(counter.current());
        })
    });
}

fn benchmark_message_buffer(c: &mut Criterion) {
    let buffer = MessageBuffer::<65536>::new();

    c.bench_function("buffer_reserve", |b| {
        // Reset buffer for each iteration
        b.iter(|| {
            buffer.reserve(black_box(1024))
        })
    });
}

fn benchmark_network_gateway_integration(c: &mut Criterion) {
    let gateway = NetworkGateway::new(1000);
    gateway.start().unwrap();

    c.bench_function("session_create", |b| {
        b.iter(|| {
            let session_id = gateway.sessions.create_session().unwrap();
            gateway.sessions.remove_session(session_id).unwrap();
        })
    });

    // Simulate high-frequency trading scenario
    c.bench_function("trading_scenario", |b| {
        b.iter(|| {
            // Create session
            let session_id = gateway.sessions.create_session().unwrap();

            // Send order
            let _order_seq = gateway.orders.send_order(session_id as u32, b"BUY 100 MSFT @ 300.50");

            // Process market data
            let header = MessageHeader::new(MessageType::MarketData, 32, session_id as u32, 1);
            let header_bytes = unsafe {
                std::slice::from_raw_parts(
                    &header as *const _ as *const u8,
                    std::mem::size_of::<MessageHeader>(),
                )
            };
            let _md_seq = gateway.market_data.process_market_data(header_bytes);

            // Clean up
            gateway.sessions.remove_session(session_id).unwrap();
        })
    });
}

criterion_group!(
    benches,
    benchmark_order_processing,
    benchmark_market_data_processing,
    benchmark_generation_counter,
    benchmark_message_buffer,
    benchmark_network_gateway_integration
);
criterion_main!(benches);