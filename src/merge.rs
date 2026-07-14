use crate::model::{Heat, Lane, Meet, SeedTime};

pub struct MixedHeatSource {
    pub event_number: u32,
    pub heat_number: u32,
}

pub struct MixedHeat {
    pub header: String,
    pub sources: Vec<MixedHeatSource>,
    pub lanes: Vec<Lane>,
}

impl MixedHeat {
    // Where this mixed heat naturally falls in program order: right after
    // the earliest event it draws from, before that event's later heats
    // finish, and before its later source event(s) begin.
    pub fn anchor_event(&self) -> u32 {
        self.sources
            .iter()
            .map(|s| s.event_number)
            .min()
            .unwrap_or(0)
    }
}

pub fn infer_lane_capacity(meet: &Meet) -> u32 {
    meet.events
        .iter()
        .flat_map(|e| &e.heats)
        .flat_map(|h| &h.lanes)
        .map(|l| l.number)
        .max()
        .unwrap_or(0)
}

pub fn heat_swimmer_count(heat: &Heat) -> usize {
    heat.lanes.iter().filter(|l| l.swimmer.is_some()).count()
}

pub fn is_heat_eligible(heat: &Heat, capacity: u32) -> bool {
    heat_swimmer_count(heat) < capacity as usize
}

pub fn can_merge(heats: &[&Heat], capacity: u32) -> bool {
    heats.len() >= 2
        && heats.iter().map(|h| heat_swimmer_count(h)).sum::<usize>() <= capacity as usize
}

/// Fastest-to-slowest lane order: right-of-center first, then alternating
/// outward. Reproduces the canonical [4, 3, 5, 2, 6, 1] for a 6-lane pool.
pub fn center_out_lane_order(lane_count: u32) -> Vec<u32> {
    let mut order = Vec::with_capacity(lane_count as usize);
    let mut right = lane_count / 2 + 1;
    let mut left = lane_count / 2;
    loop {
        let mut pushed = false;
        if right <= lane_count {
            order.push(right);
            right += 1;
            pushed = true;
        }
        if left >= 1 {
            order.push(left);
            left -= 1;
            pushed = true;
        }
        if !pushed {
            break;
        }
    }
    order
}

pub fn suggested_header(sources: &[MixedHeatSource]) -> String {
    let parts: Vec<String> = sources
        .iter()
        .map(|s| format!("event {}, heat {}", s.event_number, s.heat_number))
        .collect();
    let joined = match parts.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {}", rest.join(", "), last),
        _ => parts.join(", "),
    };
    format!("Mixed heat: {joined}")
}

// NoTime sorts after every real time; SeedTime's derived PartialOrd puts
// NoTime first, which is the opposite of what a merged heat needs.
fn seed_key(seed_time: SeedTime) -> f64 {
    match seed_time {
        SeedTime::NoTime => f64::INFINITY,
        SeedTime::Seconds(secs) => secs,
    }
}

pub fn build_mixed_heat(sources: Vec<(MixedHeatSource, &Heat)>, capacity: u32) -> MixedHeat {
    let header_sources: Vec<MixedHeatSource> = sources
        .iter()
        .map(|(s, _)| MixedHeatSource {
            event_number: s.event_number,
            heat_number: s.heat_number,
        })
        .collect();
    let header = suggested_header(&header_sources);

    let mut swimmers: Vec<_> = sources
        .into_iter()
        .flat_map(|(_, heat)| heat.lanes.iter().filter_map(|l| l.swimmer.clone()))
        .collect();
    swimmers.sort_by(|a, b| {
        seed_key(a.seed_time)
            .partial_cmp(&seed_key(b.seed_time))
            .unwrap()
    });

    let mut lanes: Vec<Lane> = swimmers
        .into_iter()
        .zip(center_out_lane_order(capacity))
        .map(|(swimmer, number)| Lane {
            number,
            swimmer: Some(swimmer),
        })
        .collect();
    lanes.sort_by_key(|l| l.number);

    MixedHeat {
        header,
        sources: header_sources,
        lanes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Swimmer;

    #[test]
    fn center_out_six_lanes_matches_canonical_order() {
        assert_eq!(center_out_lane_order(6), vec![4, 3, 5, 2, 6, 1]);
    }

    #[test]
    fn center_out_is_a_permutation_for_various_lane_counts() {
        for n in 1..=12u32 {
            let mut order = center_out_lane_order(n);
            order.sort();
            assert_eq!(order, (1..=n).collect::<Vec<_>>());
        }
    }

    #[test]
    fn suggested_header_two_sources() {
        let sources = vec![
            MixedHeatSource {
                event_number: 1,
                heat_number: 2,
            },
            MixedHeatSource {
                event_number: 2,
                heat_number: 1,
            },
        ];
        assert_eq!(
            suggested_header(&sources),
            "Mixed heat: event 1, heat 2 and event 2, heat 1"
        );
    }

    #[test]
    fn suggested_header_three_sources() {
        let sources = vec![
            MixedHeatSource {
                event_number: 1,
                heat_number: 1,
            },
            MixedHeatSource {
                event_number: 2,
                heat_number: 1,
            },
            MixedHeatSource {
                event_number: 3,
                heat_number: 1,
            },
        ];
        assert_eq!(
            suggested_header(&sources),
            "Mixed heat: event 1, heat 1, event 2, heat 1 and event 3, heat 1"
        );
    }

    #[test]
    fn anchor_event_is_the_earliest_source_event() {
        let mixed = MixedHeat {
            header: String::new(),
            sources: vec![
                MixedHeatSource {
                    event_number: 5,
                    heat_number: 1,
                },
                MixedHeatSource {
                    event_number: 2,
                    heat_number: 1,
                },
            ],
            lanes: Vec::new(),
        };
        assert_eq!(mixed.anchor_event(), 2);
    }

    fn swimmer(name: &str, seed_time: SeedTime) -> Swimmer {
        Swimmer {
            last_name: name.to_string(),
            first_name: "Test".to_string(),
            age: 10,
            exhibition: false,
            team: "TST".to_string(),
            seed_time,
        }
    }

    #[test]
    fn build_mixed_heat_seeds_fastest_in_center_and_no_time_outermost() {
        let heat_a = Heat {
            number: 1,
            of: 1,
            lanes: vec![
                Lane {
                    number: 1,
                    swimmer: Some(swimmer("Slow", SeedTime::Seconds(40.0))),
                },
                Lane {
                    number: 2,
                    swimmer: Some(swimmer("Fastest", SeedTime::Seconds(20.0))),
                },
            ],
        };
        let heat_b = Heat {
            number: 1,
            of: 1,
            lanes: vec![
                Lane {
                    number: 1,
                    swimmer: Some(swimmer("NoTime", SeedTime::NoTime)),
                },
                Lane {
                    number: 2,
                    swimmer: Some(swimmer("Middle", SeedTime::Seconds(30.0))),
                },
            ],
        };
        let sources = vec![
            (
                MixedHeatSource {
                    event_number: 1,
                    heat_number: 1,
                },
                &heat_a,
            ),
            (
                MixedHeatSource {
                    event_number: 2,
                    heat_number: 1,
                },
                &heat_b,
            ),
        ];

        let mixed = build_mixed_heat(sources, 6);

        assert_eq!(mixed.lanes.len(), 4);
        let by_lane = |n: u32| {
            mixed
                .lanes
                .iter()
                .find(|l| l.number == n)
                .and_then(|l| l.swimmer.as_ref())
                .map(|s| s.last_name.as_str())
        };
        assert_eq!(by_lane(4), Some("Fastest"));
        assert_eq!(by_lane(3), Some("Middle"));
        assert_eq!(by_lane(5), Some("Slow"));
        assert_eq!(by_lane(2), Some("NoTime"));
    }
}
