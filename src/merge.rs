use crate::model::{Heat, Lane, Meet, SeedTime};

pub struct MixedHeatSource {
    pub event_number: u32,
    pub heat_number: u32,
    pub gender: String,
    pub distance_m: u32,
    pub stroke: String,
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

// "#1/2", plus " Boys/Girls" if the sources' events differ in gender, plus
// " {min}-{max}" if the merged swimmers' ages actually vary, plus the
// distance/stroke (taken from the first source, since a mixed heat only
// makes sense when every source event races the same distance and stroke).
pub fn suggested_header(sources: &[MixedHeatSource], ages: &[u32]) -> String {
    let mut event_numbers: Vec<u32> = sources.iter().map(|s| s.event_number).collect();
    event_numbers.sort_unstable();
    event_numbers.dedup();
    let numbers = event_numbers
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("/");

    let mut genders: Vec<&str> = sources.iter().map(|s| s.gender.as_str()).collect();
    genders.sort_unstable();
    genders.dedup();

    let mut parts = vec![format!("#{numbers}")];
    if genders.len() > 1 {
        parts.push(genders.join("/"));
    }
    if let (Some(min), Some(max)) = (ages.iter().min(), ages.iter().max()) {
        if min != max {
            parts.push(format!("{min}-{max}"));
        }
    }
    if let Some(first) = sources.first() {
        parts.push(format!("{}m {}", first.distance_m, first.stroke));
    }
    parts.join(" ")
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
            gender: s.gender.clone(),
            distance_m: s.distance_m,
            stroke: s.stroke.clone(),
        })
        .collect();

    let mut swimmers: Vec<_> = sources
        .into_iter()
        .flat_map(|(_, heat)| heat.lanes.iter().filter_map(|l| l.swimmer.clone()))
        .collect();
    swimmers.sort_by(|a, b| {
        seed_key(a.seed_time)
            .partial_cmp(&seed_key(b.seed_time))
            .unwrap()
    });

    let ages: Vec<u32> = swimmers.iter().map(|s| s.age).collect();
    let header = suggested_header(&header_sources, &ages);

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

    fn source(event_number: u32, heat_number: u32, gender: &str) -> MixedHeatSource {
        MixedHeatSource {
            event_number,
            heat_number,
            gender: gender.to_string(),
            distance_m: 25,
            stroke: "Freestyle".to_string(),
        }
    }

    #[test]
    fn suggested_header_same_gender_same_age() {
        let sources = vec![source(1, 2, "Boys"), source(2, 1, "Boys")];
        assert_eq!(suggested_header(&sources, &[10, 10]), "#1/2 25m Freestyle");
    }

    #[test]
    fn suggested_header_mixed_gender() {
        let sources = vec![source(1, 2, "Boys"), source(2, 1, "Girls")];
        assert_eq!(
            suggested_header(&sources, &[10, 10]),
            "#1/2 Boys/Girls 25m Freestyle"
        );
    }

    #[test]
    fn suggested_header_mixed_ages() {
        let sources = vec![source(1, 2, "Boys"), source(2, 1, "Boys")];
        assert_eq!(
            suggested_header(&sources, &[8, 12]),
            "#1/2 8-12 25m Freestyle"
        );
    }

    #[test]
    fn suggested_header_mixed_gender_and_ages() {
        let sources = vec![source(1, 2, "Boys"), source(2, 1, "Girls")];
        assert_eq!(
            suggested_header(&sources, &[8, 12]),
            "#1/2 Boys/Girls 8-12 25m Freestyle"
        );
    }

    #[test]
    fn anchor_event_is_the_earliest_source_event() {
        let mixed = MixedHeat {
            header: String::new(),
            sources: vec![source(5, 1, "Boys"), source(2, 1, "Boys")],
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
        let sources = vec![(source(1, 1, "Boys"), &heat_a), (source(2, 1, "Boys"), &heat_b)];

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
