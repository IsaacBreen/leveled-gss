from __future__ import annotations

import json
import random
import unittest
from dataclasses import dataclass

import weighted_gss
from weighted_gss import WeightedGSS


@dataclass(frozen=True)
class Bits:
    value: int

    def join(self, other: "Bits") -> "Bits":
        return Bits(self.value | other.value)


class UnhashableBits:
    __hash__ = None

    def __init__(self, value: int) -> None:
        self.value = value

    def __eq__(self, other: object) -> bool:
        return isinstance(other, UnhashableBits) and self.value == other.value

    def join(self, other: "UnhashableBits") -> "UnhashableBits":
        return UnhashableBits(self.value | other.value)


class BrokenJoin:
    def join(self, other: "BrokenJoin") -> "BrokenJoin":
        raise RuntimeError("join failed")


class BrokenEquality:
    def __hash__(self) -> int:
        return 7

    def __eq__(self, other: object) -> bool:
        raise RuntimeError("equality failed")


def canonical(entries):
    result = {}
    for stack, weight in entries:
        key = tuple(stack)
        result[key] = weight if key not in result else result[key].join(weight)
    return result


def materialize(gss: WeightedGSS):
    return canonical(gss.to_stacks(1_000_000))


class WeightedGSSTest(unittest.TestCase):
    def test_version_and_docs(self):
        self.assertEqual(weighted_gss.__version__, "0.3.0")
        self.assertIn("persistent", WeightedGSS.__doc__.lower())
        self.assertIn("OverflowError", WeightedGSS.to_stacks.__doc__)

    def test_construction_and_persistent_stack_operations(self):
        original = WeightedGSS.from_unweighted([[1, 2], [1, 3]])
        pushed = original.push(4)
        self.assertEqual(
            {tuple(stack) for stack, weight in pushed.to_stacks()},
            {(1, 2, 4), (1, 3, 4)},
        )
        self.assertEqual(
            {tuple(stack) for stack, weight in original.to_stacks()},
            {(1, 2), (1, 3)},
        )
        self.assertTrue(all(weight is None for _, weight in original.to_stacks()))
        self.assertIsNone(original.joined_weight())
        with self.assertRaises(ValueError):
            WeightedGSS().joined_weight()
        self.assertEqual(original.tops(), {2, 3})
        self.assertEqual(original.popn(1).tops(), {1})
        with self.assertRaises(ValueError):
            original.top()
        with self.assertRaises(ValueError):
            WeightedGSS().top()

    def test_none_remains_a_valid_top_symbol(self):
        gss = WeightedGSS.from_unweighted([[None]])
        self.assertIsNone(gss.top())
        self.assertEqual(gss.tops(), {None})

    def test_top_selection_and_branches(self):
        gss = WeightedGSS.from_unweighted([[], [1, 2], [1, 3]])
        self.assertTrue(gss.has_empty_stack())
        self.assertIsNone(gss.empty_weight())
        self.assertEqual(gss.retain_empty().to_stacks(), [([], None)])
        self.assertEqual(gss.retain_top(2).to_stacks(), [([1, 2], None)])
        self.assertEqual(gss.pop_top(2).to_stacks(), [([1], None)])
        branches = {top: remainder for top, remainder in gss.pop_branches()}
        self.assertEqual(set(branches), {2, 3})
        self.assertEqual(branches[2].to_stacks(), [([1], None)])
        self.assertEqual(branches[3].to_stacks(), [([1], None)])
        with self.assertRaises(ValueError):
            WeightedGSS.from_unweighted([[1]]).empty_weight()

    def test_weighted_collisions_and_unhashable_weights(self):
        gss = WeightedGSS.from_stacks(
            [([1, 2], Bits(1)), ([1, 2], Bits(4)), ([1, 3], Bits(2))]
        )
        self.assertEqual(materialize(gss), {(1, 2): Bits(5), (1, 3): Bits(2)})
        self.assertEqual(gss.joined_weight(), Bits(7))

        unhashable = WeightedGSS.from_stacks(
            [([1], UnhashableBits(1)), ([1], UnhashableBits(2))]
        )
        [(stack, weight)] = unhashable.to_stacks()
        self.assertEqual(stack, [1])
        self.assertEqual(weight, UnhashableBits(3))

    def test_generators_with_stack_and_merge_all(self):
        gss = WeightedGSS.from_unweighted(([index] for index in range(3)))
        gss = gss.with_stack([3])
        merged = WeightedGSS.merge_all([gss.retain_top(0), gss.retain_top(3)])
        self.assertEqual(
            {tuple(stack) for stack, _ in merged.to_stacks()}, {(0,), (3,)}
        )

    def test_callback_exceptions_propagate_without_panicking(self):
        with self.assertRaisesRegex(RuntimeError, "join failed"):
            WeightedGSS.from_stacks([([1], BrokenJoin()), ([1], BrokenJoin())])

        left = WeightedGSS.from_unweighted([[BrokenEquality()]])
        right = WeightedGSS.from_unweighted([[BrokenEquality()]])
        with self.assertRaisesRegex(RuntimeError, "equality failed"):
            left.merge(right)

        with self.assertRaises(TypeError):
            WeightedGSS.from_stack([1], object())

    def test_randomized_core_operations_match_explicit_model(self):
        for seed in range(20):
            rng = random.Random(seed)
            entries = [
                (
                    [rng.randrange(6) for _ in range(rng.randrange(7))],
                    Bits(1 << rng.randrange(8)),
                )
                for _ in range(rng.randrange(16))
            ]
            model = canonical(entries)
            gss = WeightedGSS.from_stacks(entries)

            for _ in range(60):
                operation = rng.randrange(5)
                if operation == 0:
                    value = rng.randrange(6)
                    model = canonical(
                        (list(stack) + [value], weight)
                        for stack, weight in model.items()
                    )
                    gss = gss.push(value)
                elif operation == 1:
                    count = rng.randrange(5)
                    model = canonical(
                        (list(stack[:-count] if count else stack), weight)
                        for stack, weight in model.items()
                        if len(stack) >= count
                    )
                    gss = gss.popn(count)
                elif operation == 2:
                    value = rng.randrange(6)
                    model = {
                        stack: weight
                        for stack, weight in model.items()
                        if stack and stack[-1] == value
                    }
                    gss = gss.retain_top(value)
                elif operation == 3:
                    value = rng.randrange(6)
                    model = canonical(
                        (list(stack[:-1]), weight)
                        for stack, weight in model.items()
                        if stack and stack[-1] == value
                    )
                    gss = gss.pop_top(value)
                else:
                    other = [
                        (
                            [rng.randrange(6) for _ in range(rng.randrange(7))],
                            Bits(1 << rng.randrange(8)),
                        )
                        for _ in range(rng.randrange(8))
                    ]
                    model = canonical(list(model.items()) + other)
                    gss = gss.merge(WeightedGSS.from_stacks(other))

                self.assertEqual(materialize(gss), model)
                self.assertEqual(gss.is_empty(), not model)
                self.assertEqual(gss.max_depth(), max(map(len, model), default=0))

    def test_materialization_limit(self):
        gss = WeightedGSS.from_stack([], Bits(1))
        for level in range(12):
            gss = WeightedGSS.merge_all(
                [gss.push(level * 2), gss.push(level * 2 + 1)]
            )
        with self.assertRaises(OverflowError):
            gss.to_stacks(max_stacks=100)
        self.assertEqual(len(gss.to_stacks(max_stacks=1 << 12)), 1 << 12)

    def test_rejects_unhashable_stack_values_and_negative_pop(self):
        with self.assertRaises(TypeError):
            WeightedGSS.from_unweighted([[[1, 2]]])
        with self.assertRaises(ValueError):
            WeightedGSS.from_unweighted([[1]]).popn(-1)

    def test_private_structure_dump_preserves_graph_identity_and_variants(self):
        gss = WeightedGSS.from_stacks(
            [
                ([0, 4, 9, 12, 18, 31], Bits(1)),
                ([0, 4, 9, 12, 27, 31], Bits(1)),
                ([0, 4, 13, 27, 31], Bits(2)),
            ]
        )

        dump = gss._dump_structure()
        node_ids = {node["id"] for node in dump["nodes"]}
        variants = {(node["enum"], node["variant"]) for node in dump["nodes"]}

        self.assertEqual(dump["schema"], "weighted-gss/internal-structure/v1")
        self.assertIn(dump["root"], node_ids)
        self.assertTrue(all(edge["from"] in node_ids for edge in dump["edges"]))
        self.assertTrue(all(edge["to"] in node_ids for edge in dump["edges"]))
        self.assertIn(("WKind", "Branch"), variants)
        self.assertIn(("WKind", "Shared"), variants)
        self.assertIn(("UKind", "Segment"), variants)
        self.assertTrue(dump["weights"])

        terminal_nodes = [
            node
            for node in dump["nodes"]
            if node["enum"] == "UKind"
            and node["variant"] == "Branch"
            and node.get("empty") is True
        ]
        self.assertEqual(len(terminal_nodes), 1)
        terminal_id = terminal_nodes[0]["id"]
        self.assertEqual(
            sum(
                edge["kind"] == "segment_next" and edge["to"] == terminal_id
                for edge in dump["edges"]
            ),
            3,
        )

        encoded = json.loads(gss._dump_json())
        self.assertEqual(encoded["schema"], dump["schema"])
        self.assertEqual(encoded["root"], dump["root"])

    def test_private_structure_dump_preserves_graph_identity_and_variants(self):
        gss = WeightedGSS.from_stacks(
            [
                ([0, 4, 9, 12, 18, 31], Bits(1)),
                ([0, 4, 9, 12, 27, 31], Bits(1)),
                ([0, 4, 13, 27, 31], Bits(2)),
            ]
        )

        dump = gss._dump_structure()
        node_ids = {node["id"] for node in dump["nodes"]}
        variants = {(node["enum"], node["variant"]) for node in dump["nodes"]}

        self.assertEqual(dump["schema"], "weighted-gss/internal-structure/v1")
        self.assertIn(dump["root"], node_ids)
        self.assertTrue(all(edge["from"] in node_ids for edge in dump["edges"]))
        self.assertTrue(all(edge["to"] in node_ids for edge in dump["edges"]))
        self.assertIn(("WKind", "Branch"), variants)
        self.assertIn(("WKind", "Shared"), variants)
        self.assertIn(("UKind", "Segment"), variants)
        self.assertTrue(dump["weights"])

        terminal_nodes = [
            node
            for node in dump["nodes"]
            if node["enum"] == "UKind"
            and node["variant"] == "Branch"
            and node.get("empty") is True
        ]
        self.assertEqual(len(terminal_nodes), 1)
        terminal_id = terminal_nodes[0]["id"]
        self.assertEqual(
            sum(
                edge["kind"] == "segment_next" and edge["to"] == terminal_id
                for edge in dump["edges"]
            ),
            3,
        )

        encoded = json.loads(gss._dump_json())
        self.assertEqual(encoded["schema"], dump["schema"])
        self.assertEqual(encoded["root"], dump["root"])


if __name__ == "__main__":
    unittest.main()
