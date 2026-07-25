from typing import List, Set, Tuple

from weighted_gss import WeightedGSS

stacks: WeightedGSS = WeightedGSS.from_unweighted([[1, 2], [1, 3]])
tops: Set[object] = stacks.tops()
branches: List[Tuple[object, WeightedGSS]] = stacks.pop_branches()
assert tops == {2, 3}
assert len(branches) == 2
