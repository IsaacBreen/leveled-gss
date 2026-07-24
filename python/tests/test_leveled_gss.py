from __future__ import annotations

import random
import unittest
from dataclasses import dataclass

import leveled_gss
from leveled_gss import LeveledGSS, LeveledGSSSummary



@dataclass(frozen=True)
class Acc:
    bits: int

    def merge(self, other: "Acc") -> "Acc":
        return Acc(self.bits | other.bits)


def canonical(stacks):
    result = {}
    for stack, accumulator in stacks:
        key = tuple(stack)
        if key in result:
            result[key] = result[key].merge(accumulator)
        else:
            result[key] = accumulator
    return result


def materialize(gss: LeveledGSS):
    return canonical(gss.to_stacks(1_000_000))


class LeveledGSSTest(unittest.TestCase):
    def test_version_typing_and_runtime_docs(self):
        self.assertEqual(leveled_gss.__version__, "0.1.0")
        self.assertIn("persistent", LeveledGSS.__doc__.lower())
        self.assertIn("OverflowError", LeveledGSS.to_stacks.__doc__)
        self.assertTrue(LeveledGSSSummary.__doc__)

    def test_unweighted_convenience_api(self):
        gss = LeveledGSS.from_unweighted([[1, 2], [1, 3]])
        self.assertEqual(
            {tuple(stack) for stack, accumulator in gss.push(4).to_stacks()},
            {(1, 2, 4), (1, 3, 4)},
        )
        self.assertTrue(all(accumulator is None for _, accumulator in gss.to_stacks()))
        self.assertEqual(gss.peek(), {2, 3})

    def test_outer_generators_are_accepted(self):
        gss = LeveledGSS.from_unweighted(([index] for index in range(3)))
        self.assertEqual({tuple(stack) for stack, _ in gss.to_stacks()}, {(0,), (1,), (2,)})

    def test_weighted_duplicate_paths_merge(self):
        gss = LeveledGSS.from_stacks(
            [([1, 2], Acc(1)), ([1, 2], Acc(4)), ([1, 3], Acc(2))]
        )
        self.assertEqual(
            materialize(gss),
            {(1, 2): Acc(5), (1, 3): Acc(2)},
        )
        self.assertEqual(gss.reduce_acc(), Acc(7))

    def test_randomized_operations_match_explicit_model(self):
        for seed in range(40):
            rng = random.Random(seed)
            initial = [
                ([rng.randrange(8) for _ in range(rng.randrange(9))], Acc(1 << rng.randrange(12)))
                for _ in range(rng.randrange(25))
            ]
            model = canonical(initial)
            gss = LeveledGSS.from_stacks(initial)

            for _ in range(100):
                operation = rng.randrange(5)
                if operation == 0:
                    value = rng.randrange(8)
                    model = canonical((list(stack) + [value], acc) for stack, acc in model.items())
                    gss = gss.push(value)
                elif operation == 1:
                    count = rng.randrange(6)
                    model = canonical(
                        (list(stack[:-count] if count else stack), acc)
                        for stack, acc in model.items()
                        if len(stack) >= count
                    )
                    gss = gss.popn(count)
                elif operation == 2:
                    value = None if rng.random() < 0.2 else rng.randrange(8)
                    model = {
                        stack: acc
                        for stack, acc in model.items()
                        if (not stack and value is None)
                        or (stack and value is not None and stack[-1] == value)
                    }
                    gss = gss.isolate(value)
                elif operation == 3:
                    other_items = [
                        ([rng.randrange(8) for _ in range(rng.randrange(9))], Acc(1 << rng.randrange(12)))
                        for _ in range(rng.randrange(12))
                    ]
                    model = canonical(list(model.items()) + other_items)
                    gss = gss.merge(LeveledGSS.from_stacks(other_items))
                else:
                    gss = gss.fuse(None if rng.random() < 0.3 else rng.randrange(6))

                self.assertEqual(materialize(gss), model)
                self.assertEqual(gss.is_empty(), not model)
                self.assertEqual(gss.max_depth(), max(map(len, model), default=0))
                self.assertEqual(gss.path_count_at_most(1_000_000), len(gss.to_stacks(1_000_000)))

    def test_summary_and_materialization_limit(self):
        gss = LeveledGSS.from_single_stack([], Acc(1))
        for level in range(15):
            gss = LeveledGSS.merge_many([gss.push(level * 2), gss.push(level * 2 + 1)])
        summary = gss.summary()
        self.assertIsInstance(summary, LeveledGSSSummary)
        self.assertEqual(gss.path_count_at_most(100_000), 1 << 15)
        with self.assertRaises(OverflowError):
            gss.to_stacks(100)
        self.assertEqual(len(gss.to_stacks(1 << 15)), 1 << 15)

    def test_rejects_unhashable_values(self):
        with self.assertRaises(TypeError):
            LeveledGSS.from_unweighted([[[1, 2]]])


if __name__ == "__main__":
    unittest.main()
