"""Persistent weighted graph-structured stacks.

Stack values and weights must be immutable and hashable. Weights must define
``join(other)`` and return a new weight. Use ``None`` for an unweighted GSS.
"""

from ._native import WeightedGSS, WeightedGSSSummary, __version__

__all__ = ["WeightedGSS", "WeightedGSSSummary", "__version__"]
