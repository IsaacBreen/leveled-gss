use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Labels(Vec<&'static str>);

impl Weight for Labels {
    fn join(&self, other: &Self) -> Self {
        let mut labels = self.0.clone();
        for label in &other.0 {
            if !labels.contains(label) {
                labels.push(label);
            }
        }
        Labels(labels)
    }
}

#[test]
fn merge_push_and_pop_preserve_paths_and_weights() {
    let left = WeightedGss::from_single_stack(vec![0_u8, 1, 2], Labels(vec!["left"]));
    let right = WeightedGss::from_single_stack(vec![0_u8, 1, 3], Labels(vec!["right"]));

    let merged = left.merge(&right);
    assert_eq!(merged.path_count_at_most(8), 2);
    assert_eq!(merged.peek(), [2_u8, 3].into_iter().collect());

    let pushed = merged.push(4);
    assert_eq!(pushed.peek(), [4_u8].into_iter().collect());

    let mut round_trip = pushed.pop().to_stacks(8).unwrap();
    round_trip.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        round_trip,
        vec![
            (vec![0, 1, 2], Labels(vec!["left"])),
            (vec![0, 1, 3], Labels(vec!["right"])),
        ]
    );
}

#[test]
fn duplicate_stack_weights_merge() {
    let gss = WeightedGss::from_stacks(&[
        (vec![1_u8, 2], Labels(vec!["a"])),
        (vec![1_u8, 2], Labels(vec!["b"])),
    ]);

    let stacks = gss.to_stacks(4).unwrap();
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].0, vec![1, 2]);
    assert_eq!(stacks[0].1, Labels(vec!["a", "b"]));
}
