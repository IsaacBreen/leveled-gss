"""Persistent weighted graph-structured stacks.

Values and accumulators must be hashable. Weighted accumulators should define
``merge(other)`` and return a new accumulator. Use ``None`` for unweighted GSSs.
"""

from ._native import LeveledGSS, LeveledGSSSummary

__all__ = ["LeveledGSS", "LeveledGSSSummary"]
