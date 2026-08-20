# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only


from z3 import Solver, Int, Implies, Xor, And, Distinct
from typing import TYPE_CHECKING, Optional
from collections import defaultdict
from copy import deepcopy
import logging

from mcm.utils import dfs, dfs_with_path, transitive_reduction
from mcm.types import Node
from mcm.types import Addr, HartId, InstrCoord
from mcm.memops import MemInstr, Store, Load, Amo, make_syntethic_init_store

logger = logging.getLogger(__name__)
logging.basicConfig(level=logging.CRITICAL)

# TODO:
# - transitive reduction should keep the number of edges to a minimum
# - try to use z3 solver optimizations
# -

# We could use this script to validate traces, be reading the rf-map in the
# trace if we give each store a distinct value:
# - The data to store mapping does not work for AMOs as they modify the data
# - Still not super fast


class SolveStep:
    """Store a single MCM solving step. A solving steps is the possible ordering
    of the instructions between two MCM reset handlers with equal indexes
    """

    def __init__(
        self,
        step_idx: int,
        memop_addrs: list[Addr],
        step_memops: dict[HartId, dict[InstrCoord, MemInstr]],
    ):
        # all memory locations
        self.all_addrs: list[Addr] = memop_addrs
        # filter out wrapped instr, not relevant for rf_map
        self.step_memops: dict[HartId, dict[InstrCoord, MemInstr]] = step_memops

        self.loads: dict[Node, Load | Amo] = {}
        self.stores_by_addr: dict[Addr, list[Store | Amo]] = defaultdict(list)
        self.all_idx_to_memops: dict[HartId, dict[int, MemInstr]] = {}

        # computed using solver output
        self.all_rf_maps: list[dict[Load | Amo, Optional[Store | Amo]]] = []
        self.pc_to_load_return: list[dict[Addr, int]] = []
        self.step_idx: int = step_idx  # debug

    def compute_execution_graph(self):
        """Generates the rf relation for a given program
        Uses the HERD partial order model for speed
        """
        # print(f"---> Solving for step: {self.step_idx}")
        hb_model, hb_coherence = self._initialize_solver()

        # create an rf_map structure to return later
        rf_map: dict[Load | Amo, Store | Amo | None] = {
            l: None for l in self.loads.values()
        }
        # early co pruning
        co_per_addr: dict[Addr, list[Node]] = defaultdict(list)
        # add initial store to the partial coherence dictionary
        for addr, stores in self.stores_by_addr.items():
            init_store = stores[0]
            assert init_store.hartid == -1
            co_per_addr[addr].append((init_store.hartid, init_store.idx))

        s = self._init_solver(hb_model, hb_coherence)
        all_loads = set(self.loads.keys())
        self._explore(hb_model, hb_coherence, all_loads, rf_map, co_per_addr, s)
        return self.all_rf_maps

    def _explore(
        self,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        remaining_loads: set[Node],
        rf_map: dict[Load | Amo, Store | Amo | None],
        co_per_addr: dict[Addr, list[Node]],
        s: Solver,
    ):
        depth = len(self.loads) - len(remaining_loads)
        indent = "  " * depth
        # logger.info(f"{indent}[EXPLORE] Depth {depth}, remaining loads: {len(remaining_loads)}")
        # logger.info(f"{indent}  Remaining: {sorted(remaining_loads)}")

        if not remaining_loads:
            assert s.check().r == 1
            # logger.info(f"{indent}[FINALIZE] SUCCESS - execution accepted, RF map #{len(self.all_rf_maps)}")
            self.all_rf_maps.append(
                {l: s for l, s in rf_map.items()}
            )  # store a copy to avoid reverting
            return

        # pick the current load to bind
        # load_key = min(remaining_loads, key=lambda k: (k[0], k[1]))
        # use load the the least of rf candidates
        load_key = min(
            remaining_loads,
            key=lambda k: (len(self._self_store_candidates(self.loads[k])), k[0], k[1]),
        )
        load = self.loads[load_key]
        # logger.info(f"{indent}[BINDING] Selected load L{load_key} at addr {load.get_dest()}")

        # choose potential rf candidates
        candidate_stores = self._self_store_candidates(load)
        # logger.info(f"{indent}[CANDIDATES] Found {len(candidate_stores)} store candidates:")
        # for i, store in enumerate(candidate_stores):
        # store_key = (store.hartid, store.idx)
        # logger.info(f"{indent}  {i}: S{store_key} at addr {store.get_dest()}")

        for i, store in enumerate(candidate_stores):
            store_key = (store.hartid, store.idx)
            # logger.info(f"{indent}[TRY {i}] Attempting RF: S{store_key} -> L{load_key}")

            # used to revert changes
            trail_graph = []
            trail_rf_map = []

            rf_success, rf_cycle, rf_graph_type = self._add_rf_edge(
                store, load, hb_model, hb_coherence, rf_map, trail_graph, trail_rf_map
            )
            if not rf_success:
                # logger.info(f"{indent}  [REJECTED] RF edge failed (cycle in {rf_graph_type} graph)")
                # logger.info(f"{indent}    Cycle path: {' -> '.join(str(n) for n in rf_cycle)}")
                # logger.info(f"{indent}    Current RF state: {self._format_rf_map(rf_map)}")
                self._undo(trail_graph, trail_rf_map, hb_model, hb_coherence, rf_map)
                continue

            # check that co and fr can still be ok
            if not self.update_solver(s, trail_rf_map, rf_map, trail_graph):
                self._undo(trail_graph, trail_rf_map, hb_model, hb_coherence, rf_map, s)
                continue

            # logger.info(f"{indent}  [ACCEPTED] S{store_key} -> L{load_key}, recursing...")
            # logger.info(f"{indent}    Updated RF state: {self._format_rf_map(rf_map)}")
            # if no cycle is found, go one step deeper
            next_remaining_loads = remaining_loads - {(load.hartid, load.idx)}
            self._explore(
                hb_model, hb_coherence, next_remaining_loads, rf_map, co_per_addr, s
            )
            self._undo(trail_graph, trail_rf_map, hb_model, hb_coherence, rf_map, s)
            # logger.info(f"{indent}  [RETURN] Back from S{store_key} -> L{load_key}")

        # logger.info(f"{indent}[DONE] Finished exploring all candidates for L{load_key}")

    def _init_solver(
        self,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
    ):
        """Completes the CO and FR relations and checks if both graphs are acyclic.
        CO must be total per address. FR = (load, store) where load reads-from store'
        and store' <co store.
        """
        s = Solver()
        all_nodes = [
            (h, m.idx)
            for h, memops in self.step_memops.items()
            for m in memops.values()
        ]
        for i in range(len(self.all_addrs)):
            all_nodes.append((-1, i))  # init stores

        # Separate rank variables for each graph
        self.rank_model = {n: Int(f"rk_model_{n[0]}_{n[1]}") for n in all_nodes}
        self.rank_coh = {n: Int(f"rk_coh_{n[0]}_{n[1]}") for n in all_nodes}
        n_node = len(all_nodes)

        for n in all_nodes:
            s.add(And(self.rank_model[n] >= 0, self.rank_model[n] < n_node))
            s.add(And(self.rank_coh[n] >= 0, self.rank_coh[n] < n_node))

        # enforce distinctness per graph OPTIONAL
        s.add(Distinct(*self.rank_model.values()))
        s.add(Distinct(*self.rank_coh.values()))

        # === COHERENCE GRAPH EDGES ===
        # logger.debug("\n\n=== COHERENCE GRAPH EDGES ===")
        for rel_name, adj in hb_coherence.items():
            # logger.debug(f"{rel_name}:")
            for u, succs in adj.items():
                for v in succs:
                    # logger.debug(f"  {u} -> {v}")
                    s.assert_and_track(
                        self.rank_coh[u] < self.rank_coh[v], f"coh_{rel_name}_{u}_{v}"
                    )

        # === MODEL GRAPH EDGES ===
        # logger.debug("=== MODEL GRAPH EDGES ===")
        for rel_name, adj in hb_model.items():
            # logger.debug(f"{rel_name}:")
            for u, succs in adj.items():
                for v in succs:
                    # logger.debug(f"  {u} -> {v}")
                    s.assert_and_track(
                        self.rank_model[u] < self.rank_model[v],
                        f"mod_{rel_name}_{u}_{v}",
                    )

        # === CO TOTALITY AND INIT ORDERING ===
        for init_idx, addr in enumerate(self.all_addrs):
            store_nodes = []
            for store in self.stores_by_addr[addr]:
                assert isinstance(store, (Amo, Store))
                store_nodes.append((store.hartid, store.idx))

            for i in range(len(store_nodes)):
                for j in range(i + 1, len(store_nodes)):
                    u, v = store_nodes[i], store_nodes[j]
                    s.add(
                        Xor(
                            self.rank_coh[u] < self.rank_coh[v],
                            self.rank_coh[v] < self.rank_coh[u],
                        )
                    )
                    s.add(
                        Xor(
                            self.rank_model[u] < self.rank_model[v],
                            self.rank_model[v] < self.rank_model[u],
                        )
                    )

            init_store = (-1, init_idx)
            for prog_store in store_nodes:
                if prog_store[0] != -1:
                    # logger.debug(f"  Forcing CO constraint: {init_store} < {prog_store} (initial before program)")
                    s.add(
                        self.rank_coh[init_store] < self.rank_coh[prog_store],
                    )
                    s.add(
                        self.rank_model[init_store] < self.rank_model[prog_store],
                    )
        return s

    def update_solver(
        self,
        s: Solver,
        trail_rf_map: list[Load | Amo],
        rf_map: dict[Load | Amo, Store | Amo | None],
        trail_graph: list[tuple[str, Node, Node]],
    ):
        s.push()

        # add new fr constraints
        for load in trail_rf_map:
            # === FR CONSTRAINTS ===
            # logger.debug("=== FR CONSTRAINTS ===")
            rf_source = rf_map[load]
            assert rf_source is not None
            load_node = (load.hartid, load.idx)
            rf_source_node = (rf_source.hartid, rf_source.idx)
            addr = rf_source.get_dest()
            # logger.debug(f"Load {load_node} reads from store {rf_source_node} at addr {addr}")

            for other_store in self.stores_by_addr[addr]:
                other_store_node = (other_store.hartid, other_store.idx)
                if load_node == other_store_node:
                    continue
                if other_store.hartid == -1 or other_store == rf_source:
                    continue
                if not isinstance(other_store, (Amo, Store)):
                    continue
                # logger.debug(f"  Checking store {other_store_node}")
                # Coherence graph FR constraint
                s.add(
                    Implies(
                        self.rank_coh[rf_source_node] < self.rank_coh[other_store_node],
                        self.rank_coh[load_node] < self.rank_coh[other_store_node],
                    )
                )
                # Model graph FR constraint
                s.add(
                    Implies(
                        self.rank_model[rf_source_node]
                        < self.rank_model[other_store_node],
                        self.rank_model[load_node] < self.rank_model[other_store_node],
                    )
                )

        # add new graph edges
        for rel, src, dest in trail_graph:
            if rel in ("co", "rf", "rfe", "fr", "po-loc"):
                coh_rel = "rf" if rel == "rfe" else rel
                s.add(self.rank_coh[src] < self.rank_coh[dest])
            # add an edge to the model graph
            if rel in ("co", "rfe", "fr", "ppo"):
                if src[0] != -1 and dest[0] != -1:
                    s.add(self.rank_model[src] < self.rank_model[dest])

        result = s.check()
        return result.r == 1

    def _add_rf_edge(
        self,
        store: Store | Amo,
        load: Load | Amo,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        rf_map: dict[Load | Amo, Store | Amo | None],
        trail_graph: list[tuple[str, Node, Node]],
        trail_rf_map: list[Load | Amo],
    ) -> tuple[bool, list[Node], str]:
        """add an rf edge to the graph"""
        if store.hartid == load.hartid:
            relation = "rf"
        else:
            relation = "rfe"
        store_node = (store.hartid, store.idx)
        load_node = (load.hartid, load.idx)
        success, cycle_path, graph_type = self._add_edge(
            relation, store_node, load_node, hb_model, hb_coherence, trail_graph
        )
        if not success:
            return False, cycle_path, graph_type

        # on success, update rf_map
        rf_map[load] = store
        trail_rf_map.append(load)

        # check all guarded PPO rules when adding a new RF edge
        success, cycle_path, graph_type = self._add_ppo_rule2_edges(
            hb_model, hb_coherence, rf_map, trail_graph
        )
        if not success:
            return False, cycle_path, graph_type
        success, cycle_path, graph_type = self._add_ppo_rule3_edges(
            hb_model, hb_coherence, rf_map, load, trail_graph
        )
        if not success:
            return False, cycle_path, graph_type
        success, cycle_path, graph_type = self._add_ppo_rule12_edges(
            hb_model, hb_coherence, rf_map, load, trail_graph
        )
        if not success:
            return False, cycle_path, graph_type

        return True, [], ""

    def _self_store_candidates(self, load: Load | Amo):
        """Selects all valid store candidate for the rf edges, given a load"""
        valid_candidates: list[Store | Amo] = []
        same_addr_stores = self.stores_by_addr[load.get_dest()]

        # prune early, a load can only return the previous po-loc store
        latest_same_hart_store = None
        for store in same_addr_stores:
            if (
                store.hartid == load.hartid
                and store.idx < load.idx
                and isinstance(store, (Store, Amo))
            ):
                if (
                    latest_same_hart_store is None
                    or store.idx > latest_same_hart_store.idx
                ):
                    latest_same_hart_store = store

        for store in same_addr_stores:
            assert store._check_overlap(load)
            # filter out stores that come after the load in PO
            if store.hartid == load.hartid and store.idx > load.idx:
                continue
            if store == load:
                continue  # skip amo
            # If this is the initial store (hartid == -1) and there's a same-hart store
            # that comes before the load, the initial store is not a valid candidate
            if store.hartid == -1 and latest_same_hart_store is not None:
                continue

            valid_candidates.append(store)
        return valid_candidates

    def _add_po_loc_edges(self, hb_coherence: dict[str, dict[Node, set[Node]]]):
        """add the po-loc edges to the coherence graph"""
        for memops in self.step_memops.values():
            po_memops = sorted(memops.values(), key=lambda m: m.idx)
            for idx_a, instr_a in enumerate(po_memops):
                for instr_b in po_memops[idx_a + 1 :]:
                    if instr_a._check_overlap(instr_b):
                        node_a = (instr_a.hartid, instr_a.idx)
                        node_b = (instr_b.hartid, instr_b.idx)
                        hb_coherence["po-loc"][node_a].add(node_b)
                        break

    def _add_ppo_edges(self, hb_model: dict[str, dict[Node, set[Node]]]):
        """
        Build a unified PPO adjacency list for a hart.
        Keys and values are PoIdx (program-order indices).
        Does not add PPO rule 2,3 and 12, as they are guarded edges and are
        computed lazily during generation
        """
        # build ppo graph
        for hartid, memop in self.step_memops.items():
            for instr in memop.values():
                successors = (
                    instr.rw_w_overlap
                    | instr.fence_successor
                    | instr.acquire
                    | instr.release
                    | instr.sequential
                    | instr.syntactic_data_dep
                    | instr.syntactic_addr_dep
                    | instr.syntactic_ctrl_dep
                    | instr.rule_13
                )
                pred_node = (hartid, instr.idx)
                for successor in successors:
                    # add ppo edge to coherence graph
                    succ_node = (hartid, successor)
                    hb_model["ppo"][pred_node].add(succ_node)

    def _index_loads_and_stores(self):
        """Index all loads, and saves all overlapping stores"""
        for hartid, memops in self.step_memops.items():
            for instr in memops.values():
                key = (hartid, instr.idx)
                if isinstance(instr, (Load, Amo)):
                    self.loads[key] = instr
                if isinstance(instr, (Store, Amo)):
                    assert instr.dest is not None
                    self.stores_by_addr[instr.dest].append(instr)

        for init_idx, addr in enumerate(self.all_addrs):
            init_store = make_syntethic_init_store(addr, init_idx)
            self.stores_by_addr[addr].append(init_store)
            self.stores_by_addr[addr].sort(key=lambda m: (m.hartid, m.idx))

        assert len(self.all_addrs) == len(self.stores_by_addr), (
            len(self.all_addrs),
            len(self.stores_by_addr),
        )

    def _initialize_solver(self):
        # create an idx to memops map
        for hartid, memops in self.step_memops.items():
            idx_to_memop = {m.idx: m for m in memops.values()}
            self.all_idx_to_memops[hartid] = idx_to_memop

        # coherence HB graph, acyclic co|rf|fr|po-loc as Coherence
        hb_coherence: dict[str, dict[Node, set[Node]]] = {
            "co": defaultdict(set),
            "rf": defaultdict(set),
            "fr": defaultdict(set),
            "po-loc": defaultdict(set),
        }
        # model HB graph, acyclic co|rfe|fr|ppo as Model
        hb_model: dict[str, dict[Node, set[Node]]] = {
            "co": defaultdict(set),
            "rfe": defaultdict(set),
            "fr": defaultdict(set),
            "ppo": defaultdict(set),
        }

        # add static ppo edges to the hb_model graph
        self._add_ppo_edges(hb_model)

        # add the po-loc edges to the coherence graph
        self._add_po_loc_edges(hb_coherence)

        # index loads and stores
        self._index_loads_and_stores()

        return hb_model, hb_coherence

    def _add_edge(
        self,
        relation: str,
        src: Node,
        dest: Node,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        trail_graph: list[tuple[str, Node, Node]],
    ) -> tuple[bool, list[Node], str]:
        """adds an edges to the graphs the handle the corresponding event.
        Asking for both graphs as an input makes it easier to keep in sync
        Returns (success, cycle_path_if_failed, graph_type_if_failed)
        """

        def rollback_co(coherence_edge_added):
            # Rollback coherence edge if it was added
            if coherence_edge_added:
                coh_rel = "rf" if relation == "rfe" else relation
                hb_coherence[coh_rel][src].remove(dest)
                if not hb_coherence[coh_rel][src]:
                    del hb_coherence[coh_rel][src]

        # use both rf and rfe as rf is a strict superset of rfe
        coherence_edge_added = False

        # add an edges to the coherence graph
        if relation in ("co", "rf", "rfe", "fr", "po-loc"):
            coh_rel = "rf" if relation == "rfe" else relation
            # Check if edge already exists
            if dest in hb_coherence[coh_rel].get(src, set()):
                return True, [], ""
            has_cycle, cycle_path = self.would_create_cycle(hb_coherence, src, dest)
            if has_cycle:
                return False, cycle_path, "coherence"
            hb_coherence[coh_rel][src].add(dest)
            coherence_edge_added = True
        # add an edge to the model graph
        if relation in ("co", "rfe", "fr", "ppo"):
            # Check if edge already exists
            if dest in hb_model[relation].get(src, set()):
                return True, [], ""
            has_cycle, cycle_path = self.would_create_cycle(hb_model, src, dest)
            if has_cycle:
                rollback_co(coherence_edge_added)
                return False, cycle_path, "model"
            if src[0] != -1 and dest[0] != -1:
                hb_model[relation][src].add(dest)

        trail_graph.append((relation, src, dest))
        return True, [], ""

    def would_create_cycle(
        self, graph: dict[str, dict[Node, set[Node]]], src: Node, dest: Node
    ) -> tuple[bool, list[Node]]:
        """returns true if adding an edge between src and dest would create a
        cycle. If there is a path from start to dest, the adding the edge
        creates a loop.
        If we can reach src from dest, adding the path src->dest can loop back
        to src
        Returns (has_cycle, cycle_path)
        """
        # Don't allow self-edges
        assert src != dest

        # compute union of the given graphs relation, TODO keep a union view as
        # well instead of creating it each time
        adj = defaultdict(set)
        for rel_view in graph.values():
            for src_node, dest_nodes in rel_view.items():
                adj[src_node].update(dest_nodes)
        has_cycle, path = dfs_with_path(adj, dest, src)
        if has_cycle:
            # Complete the cycle: path goes from dest to src, adding src->dest completes it
            return True, path + [dest]
        return False, []

    def _add_ppo_rule2_edges(
        self,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        rf_map: dict[Load | Amo, Store | Amo | None],
        trail_graph: list[tuple[str, Node, Node]],
    ) -> tuple[bool, list[Node], str]:
        """
        PPO rule 2: same-hart loads to overlapping bytes, no intervening store,
        different rf sources.
        """
        for memops in self.step_memops.values():
            po_memops = sorted(memops.values(), key=lambda m: m.idx)
            for a_idx, load_a in enumerate(po_memops):
                if not isinstance(load_a, (Load, Amo)):
                    continue
                for b_idx in range(a_idx + 1, len(po_memops)):
                    load_b = po_memops[b_idx]
                    # must be two overlapping loads
                    if not isinstance(load_b, (Load, Amo)):
                        continue
                    if not load_a._check_overlap(load_b):
                        continue
                    # the load must both have an rf relation
                    store_a = rf_map.get(load_a)
                    store_b = rf_map.get(load_b)
                    if store_a is None or store_b is None:
                        continue
                    # loads must read from 2 different stores
                    if store_a == store_b:
                        continue
                    # intervening store?
                    if any(
                        isinstance(instr, (Store, Amo)) and instr._check_overlap(load_a)
                        for instr in po_memops[a_idx + 1 : b_idx]
                    ):
                        continue
                    node_a = (load_a.hartid, load_a.idx)
                    node_b = (load_b.hartid, load_b.idx)
                    # for stats
                    load_a.r_r_overlap.add(load_b.idx)
                    success, cycle_path, graph_type = self._add_edge(
                        "ppo", node_a, node_b, hb_model, hb_coherence, trail_graph
                    )
                    if not success:
                        return False, cycle_path, graph_type
        return True, [], ""

    def _add_ppo_rule3_edges(
        self,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        rf_map: dict[Load | Amo, Store | Amo | None],
        load_b: Load | Amo,
        trail_graph: list[tuple[str, Node, Node]],
    ) -> tuple[bool, list[Node], str]:
        """
        PPO rule 3: If load_b reads from an AMO/SC write a in the same hart,
        add ppo edge a -> b.
        """
        # cannot return init
        memop_a = rf_map[load_b]
        if memop_a is None:
            return True, [], ""  # not bound yet
        # Gate on AMO/SC write; adapt this check to your types/flags.
        is_amo = isinstance(memop_a, Amo)
        if not is_amo:
            return True, [], ""
        # PPO is intra-thread
        if memop_a.hartid != load_b.hartid:
            return True, [], ""
        # Add the ppo edge to the model graph
        node_a = (memop_a.hartid, memop_a.idx)
        node_b = (load_b.hartid, load_b.idx)
        # for stats
        memop_a.amo_read.add(load_b.idx)
        return self._add_edge(
            "ppo", node_a, node_b, hb_model, hb_coherence, trail_graph
        )

    def _add_ppo_rule12_edges(
        self,
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        rf_map: dict[Load | Amo, Store | Amo | None],
        load_b: Load | Amo,
        trail_graph: list[tuple[str, Node, Node]],
    ) -> tuple[bool, list[Node], str]:
        def has_addr_or_data_dep(m: MemInstr, a: MemInstr):
            # if the index of m is in a.syntactic_addr_dep, it m is successor
            # of a, so m has a dep on a.
            assert m.hartid == a.hartid
            return (m.idx in a.syntactic_addr_dep) or (m.idx in a.syntactic_data_dep)

        # b returns value written by M
        store_m = rf_map[load_b]
        # skip if not bound to a store
        if store_m is None:
            return True, [], ""
        # must be on the same hart for PO
        if load_b.hartid != store_m.hartid:
            return True, [], ""
        # m must be before b in PO
        if store_m.idx >= load_b.idx:
            return True, [], ""

        # Scan backwards from m to find any a before m
        for idx_a in range(store_m.idx - 1, -1, -1):
            a = self.all_idx_to_memops[load_b.hartid][idx_a]
            if has_addr_or_data_dep(store_m, a):
                # TODO stop if barrier or so found
                # Add PPO edge a -> b
                node_a = (a.hartid, a.idx)
                node_b = (load_b.hartid, load_b.idx)
                success, cycle_path, graph_type = self._add_edge(
                    "ppo", node_a, node_b, hb_model, hb_coherence, trail_graph
                )
                # for stats
                a.rule_12.add(load_b.idx)
                if not success:
                    return False, cycle_path, graph_type  # cycle
        return True, [], ""

    def _undo(
        self,
        trail_graph: list[tuple[str, Node, Node]],
        trail_rf_map: list[Load | Amo],
        hb_model: dict[str, dict[Node, set[Node]]],
        hb_coherence: dict[str, dict[Node, set[Node]]],
        rf_map: dict[Load | Amo, Store | Amo | None],
        s: Solver | None = None,
    ):
        # remove last constraints if the solver was updated
        if s is not None:
            s.pop()

        for rel, src, dest in trail_graph:
            # delete edges added to the model graph
            if rel in hb_model.keys() and src[0] != -1 and dest[0] != -1:
                if src in hb_model[rel] and dest in hb_model[rel][src]:
                    hb_model[rel][src].remove(dest)
                    if not hb_model[rel][src]:
                        del hb_model[rel][src]
            # delete edges added to the coherence graph
            if rel in hb_coherence.keys() or rel == "rfe":
                co_rel = "rf" if rel == "rfe" else rel
                if src in hb_coherence[co_rel] and dest in hb_coherence[co_rel][src]:
                    hb_coherence[co_rel][src].remove(dest)
                    if not hb_coherence[co_rel][src]:
                        del hb_coherence[co_rel][src]

        for load in trail_rf_map:
            rf_map[load] = None

    def _format_rf_map(self, rf_map: dict[Load | Amo, Store | Amo | None]) -> str:
        """Format RF map for debug output"""
        if not rf_map:
            return "empty"

        bindings = []
        for load, store in rf_map.items():
            load_key = (load.hartid, load.idx)
            if store is None:
                bindings.append(f"L{load_key}->None")
            else:
                store_key = (store.hartid, store.idx)
                bindings.append(f"L{load_key}->S{store_key}")

        return "{" + ", ".join(bindings) + "}"
