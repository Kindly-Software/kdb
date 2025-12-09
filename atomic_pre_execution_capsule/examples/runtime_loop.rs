use atomic_pre_execution_capsule::{
    pipeline::{self, PexRouter, PexWriter},
    PexCapsule,
};

#[derive(Clone, Copy)]
struct TriggerFrame {
    mask: u16,
    description: &'static str,
}

fn main() {
    let capsule = PexCapsule::new();
    let mut writer = PexWriter::new(&capsule);
    writer.set_draft(pipeline::topstep_default_playbook());

    // Simulate a short replay stream with different trigger combinations.
    let frames = [
        TriggerFrame {
            mask: 0b0011_1111,
            description: "calm order book, reversion ready",
        },
        TriggerFrame {
            mask: 0b0001_1111,
            description: "sweep detected, momentum follow",
        },
        TriggerFrame {
            mask: 0,
            description: "locks in effect, skip",
        },
    ];

    let mut router = PexRouter::new(&capsule);

    for (idx, frame) in frames.iter().enumerate() {
        {
            let draft = writer.draft_mut();
            draft.header.created_ms_coarse += 50;
            draft.header.global_flags = if frame.mask == 0 { 0b0000_0001 } else { 0 };
        }
        writer.publish();

        println!("Frame #{idx}: {}", frame.description);
        router.for_each_play(|lane, play, snapshot| {
            let required = play.trig_mask;
            let eligible = required == 0 || (frame.mask & required) == required;
            if eligible {
                let header = snapshot.header();
                println!(
                    "  firing play lane={lane} priority={} qty={} ttl={}ms",
                    play.priority, play.qty, play.ttl_ms
                );
                println!(
                    "    account={} mask=0x{:04x}",
                    header.account_id, header.play_mask
                );
                true
            } else {
                println!(
                    "  skip lane={lane}: mask requires 0b{:016b}, frame=0b{:016b}",
                    required, frame.mask
                );
                false
            }
        });
        println!("---");
    }

    println!("Publish stats: {:?}", writer.stats());
    println!("Router stats: {:?}", router.stats());
}
