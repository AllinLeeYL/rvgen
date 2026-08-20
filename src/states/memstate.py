# Copyright 2026 Flavien Solt, Quentin Bordier ETH Zurich.
# Licensed under the General Public License, Version 3.0, see LICENSE for details.
# SPDX-License-Identifier: GPL-3.0-only

"""
This script defines the memory allocation.

For the moment, MemState is not designed to be thread-safe.
MemState is a data structure that represents allocated and free memory.
MemState is not yet designed to free any memory. Only more memory can be
further allocated.

A design assumption is that we will use the available memory very sparsely.

Internally, MemState is implemented as a sorted iterable of pairs
(free_start_addr, free_end_addr_plus_one)
Internally, it offers the guarantee that if (a, b) and (c, d) are in the
iterable in this order, then b < c (i.e., no superposition and no
juxtaposition)
"""

from random import Random

MEMVIEW_ALLOC_MAX_ATTEMPTS = 1000


class MemState:
    """
    MemState class represents the memory allocation state.

    Attributes:
        freepairs (list): A list of tuples representing free memory ranges.
        memsize (int): The total size of the memory.
        occupied_addrs (int): The number of occupied addresses.
    """

    def __init__(self, memsize: int, prng: Random):
        """
        Initializes the MemState with a given memory size.

        Args:
            memsize (int): The size of the memory. Should be at least 4,
            typically much higher. It is also typically a power of 2.
        """
        self.freepairs: list[tuple[int, int]] = [(0, memsize)]
        self.memsize: int = memsize
        self.occupied_addrs: int = 0  # Follow the number of occupied addresses.
        self.prng: Random = prng

    def is_mem_free(self, addr: int):
        """
        Checks if a specific memory address is free.

        Args:
            addr (int): The memory address to check.

        Returns:
            bool: True if the memory address is free, False otherwise.
        """
        for curr_pair in self.freepairs:
            if addr < curr_pair[1]:
                return curr_pair[0] <= addr
        return False

    def is_mem_range_free(self, start: int, end: int):
        """
        Checks if a range of memory addresses is free.

        Args:
            start (int): The first address of the range.
            end (int): The last address of the range, excluded.

        Returns:
            bool: True if the memory range is free, False otherwise.
        """
        # Find the pair to which `start` belongs, and then check that `end` is
        # still in the same pair.
        for curr_pair in self.freepairs:
            if start < curr_pair[1]:
                return start >= curr_pair[0] and end <= curr_pair[1]
        return False

    def get_available_contig_space(self, addr: int):
        """
        Gets the number of contiguous free addresses starting from a specific
        address.

        Args:
            addr (int): The starting address.

        Returns:
            int: The number of contiguous free addresses.
        """
        for curr_pair in self.freepairs:
            if addr < curr_pair[1]:
                if addr >= curr_pair[0]:
                    return curr_pair[1] - addr
                else:
                    return 0
        return 0

    def alloc_mem_range(self, start: int, alloc_size: int):
        """
        Allocates a range of memory addresses.

        Args:
            start (int): The first address of the range.
            alloc_size (int): The size of the memory region to allocate,
                excluding the last address.

        Raises:
            ValueError: If the memory range to allocate is not free.
        """
        end = start + alloc_size
        assert end < self.memsize
        if __debug__:
            assert end > start, (
                f"Expected start ({start}) > end ({end}) in alloc_mem_range."
            )
        self.occupied_addrs += end - start
        for curr_pair_id, curr_pair in enumerate(self.freepairs):
            if start < curr_pair[1]:
                # Check that the range is initially free.
                if __debug__:
                    assert start >= curr_pair[0] and end <= curr_pair[1], (
                        "The memory range to allocate is not free."
                    )
                # Remove the tuple and replace it with at most two smaller
                # tuples. This will automatically coalesce.
                if start == curr_pair[0] and end == curr_pair[1]:
                    self.freepairs = (
                        self.freepairs[:curr_pair_id]
                        + self.freepairs[curr_pair_id + 1 :]
                    )
                    break
                elif start == curr_pair[0]:
                    self.freepairs = (
                        self.freepairs[:curr_pair_id]
                        + [(end, curr_pair[1])]
                        + self.freepairs[curr_pair_id + 1 :]
                    )
                    break
                elif end == curr_pair[1]:
                    self.freepairs = (
                        self.freepairs[:curr_pair_id]
                        + [(curr_pair[0], start)]
                        + self.freepairs[curr_pair_id + 1 :]
                    )
                    break
                else:
                    self.freepairs = (
                        self.freepairs[:curr_pair_id]
                        + [(curr_pair[0], start)]
                        + [(end, curr_pair[1])]
                        + self.freepairs[curr_pair_id + 1 :]
                    )
                    break
        else:
            raise ValueError(
                "Trying to allocate a memory range that was already not free."
            )

    def gen_random_free_addr(
        self,
        alignment_bits: int,
        min_space: int,
        left_bound: int,
        right_bound: int,
        max_attempts: int = MEMVIEW_ALLOC_MAX_ATTEMPTS,
    ):
        """
        Generates a random free memory address within specified bounds and
        alignment.

        Args:
            alignment_bits (int): Bits of alignment (e.g., 0 for no specific
                alignment, 1 for 2-byte alignment, etc.).
            min_space (int): The minimal number of memory addresses that are
                free, starting from the returned address.
            left_bound (int): The left bound of the address range (included).
            right_bound (int): The right bound of the address range (excluded).
            max_attempts (int): The maximum number of random attempts. Must be
                strictly positive.

        Returns:
            int or None: The generated address if found, None otherwise.
        """
        left_bound = max(left_bound, 0)
        right_bound = min(right_bound, self.memsize)
        if __debug__:
            assert max_attempts > 0
            assert min_space >= 0
            assert left_bound >= 0
            assert right_bound <= self.memsize
            assert left_bound < right_bound
            # The bounds must be sufficiently spaced. In our use case, this is
            # not at all a problem.
            assert ((left_bound + (1 << alignment_bits) - 1) >> alignment_bits) < (
                (right_bound - min_space) >> alignment_bits
            )

        # TODO we could avoid looping probably
        for _ in range(max_attempts):
            picked_addr = (
                self.prng.randrange(
                    (left_bound + (1 << alignment_bits) - 1) >> alignment_bits,
                    ((right_bound - min_space) >> alignment_bits),
                )
                << alignment_bits
            )
            if (
                min_space == 0
                or self.is_mem_range_free(picked_addr, picked_addr + min_space)
            ):  # is_mem_range_free returns False if it goes beyond the memory boundaries.
                if __debug__:
                    assert picked_addr >= 0
                    assert picked_addr + min_space <= self.memsize
                    assert picked_addr % (1 << alignment_bits) == 0
                return picked_addr
        return None

    def get_allocated_ratio(self):
        """
        Computes the percentage of the memory that is allocated.

        Returns:
            float: The percentage of allocated memory.
        """
        free_sum = sum(map(lambda p: p[1] - p[0], self.freepairs))
        return (self.memsize - free_sum) / self.memsize
