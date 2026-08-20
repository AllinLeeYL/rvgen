# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only
from mcm.types import Node


def dfs(adj: dict[Node, set[Node]], start: Node, target: Node) -> bool:
    """Depth-first-search, returns true if a path from start to target
    exists
    """
    # TODO maybe change for topological sort
    stack = [start]
    visited = set()
    while stack:
        node = stack.pop()
        if node == target:
            return True
        if node in visited:
            continue
        visited.add(node)
        stack.extend(adj.get(node, []))
    return False


def dfs_with_path(
    adj: dict[Node, set[Node]], start: Node, target: Node
) -> tuple[bool, list[Node]]:
    """Depth-first-search that returns both whether a path exists and the path itself"""
    if start == target:
        return True, [start]

    # Use a stack that stores (node, path_to_node)
    stack = [(start, [start])]
    visited = set()

    while stack:
        node, path = stack.pop()
        if node in visited:
            continue
        visited.add(node)

        for neighbor in adj.get(node, []):
            if neighbor == target:
                return True, path + [neighbor]
            if neighbor not in visited and neighbor not in path:  # Avoid cycles in path
                stack.append((neighbor, path + [neighbor]))

    return False, []


def transitive_reduction(adj: dict[Node, set[Node]]) -> dict[Node, set[Node]]:
    """Performs transitive reduction on a directed acyclic graph.

    Removes edges (u,v) if there exists a path from u to v that doesn't use the direct edge.
    This reduces redundancy while preserving reachability properties.

    Args:
        adj: Adjacency list representation of the graph

    Returns:
        New adjacency list with redundant edges removed
    """
    # Create a copy to avoid modifying the original
    reduced_adj: dict[Node, set[Node]] = {}
    for node, neighbors in adj.items():
        reduced_adj[node] = neighbors.copy()

    # For each edge (u, v), check if there's an alternate path from u to v
    for u in list(reduced_adj.keys()):
        if u not in reduced_adj:
            continue

        for v in list(reduced_adj[u]):
            # Temporarily remove the direct edge u -> v
            reduced_adj[u].discard(v)

            # Check if v is still reachable from u without the direct edge
            if dfs(reduced_adj, u, v):
                # v is still reachable, so the direct edge is redundant
                pass  # Keep the edge removed
            else:
                # v is not reachable without the direct edge, so we need it
                reduced_adj[u].add(v)

    return reduced_adj
