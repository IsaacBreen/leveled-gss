use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Paths(Vec<&'static str>);

impl Weight for Paths {
    fn join(&self, other: &Self) -> Self {
        let mut paths = self.0.clone();
        for path in &other.0 {
            if !paths.contains(path) {
                paths.push(path);
            }
        }
        Paths(paths)
    }
}

fn main() {
    let left = WeightedGss::from_single_stack(vec![0_u32, 10, 20], Paths(vec!["left"]));
    let right = WeightedGss::from_single_stack(vec![0_u32, 10, 30], Paths(vec!["right"]));
    let merged = left.merge(&right).push(40);

    println!("{:#?}", merged.summary());
    for (stack, weight) in merged.to_stacks(16).expect("too many paths") {
        println!("{stack:?} => {weight:?}");
    }
}
