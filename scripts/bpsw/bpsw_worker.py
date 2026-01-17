#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Distributed BPSW worker for Newral.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import time
from datetime import datetime, timezone
from typing import Dict, Iterable, List, Optional, Tuple

try:
    import gmpy2  # type: ignore
except Exception:
    gmpy2 = None  # type: ignore


SMALL_PRIMES = [
    2,
    3,
    5,
    7,
    11,
    13,
    17,
    19,
    23,
    29,
    31,
    37,
]

DEFAULT_M_PRIMES = [13, 17, 29, 37, 41]
DEFAULT_N_PRIMES = [3, 7, 11, 19, 23]
DEFAULT_LAMBDA_FACTORS = [(2, 10), (3, 6), (5, 4), (7, 3), (11, 2), (13, 2), (17, 1)]


def is_square(n: int) -> bool:
    if n < 0:
        return False
    root = int(math.isqrt(n))
    return root * root == n


def jacobi(a: int, n: int) -> int:
    if n <= 0 or n % 2 == 0:
        return 0
    a = a % n
    result = 1
    while a != 0:
        while a % 2 == 0:
            a //= 2
            if n % 8 in (3, 5):
                result = -result
        a, n = n, a
        if a % 4 == 3 and n % 4 == 3:
            result = -result
        a %= n
    return result if n == 1 else 0


def miller_rabin(n: int, bases: Iterable[int]) -> bool:
    if n < 2:
        return False
    if n % 2 == 0:
        return n == 2
    d = n - 1
    s = 0
    while d % 2 == 0:
        d //= 2
        s += 1
    for a in bases:
        if a % n == 0:
            continue
        x = pow(a, d, n)
        if x in (1, n - 1):
            continue
        composite = True
        for _ in range(s - 1):
            x = pow(x, 2, n)
            if x == n - 1:
                composite = False
                break
        if composite:
            return False
    return True


def lucas_selfridge(n: int) -> bool:
    if n < 2 or n % 2 == 0:
        return n == 2
    if is_square(n):
        return False

    d = 5
    sign = 1
    while True:
        j = jacobi(d, n)
        if j == -1:
            break
        d += 2
        sign = -sign
        d = sign * d

    p = 1
    q = (1 - d) // 4

    s = 0
    n_plus_one = n + 1
    while n_plus_one % 2 == 0:
        n_plus_one //= 2
        s += 1

    u = 1
    v = p
    q_k = q
    d_bit = n_plus_one
    while d_bit > 1:
        if d_bit % 2 == 1:
            u = (u * v) % n
            v = (v * v - 2 * q_k) % n
        else:
            v = (v * v - 2 * q_k) % n
        q_k = (q_k * q_k) % n
        d_bit //= 2

    if u == 0 or v == 0:
        return True
    for _ in range(s - 1):
        v = (v * v - 2 * q_k) % n
        q_k = (q_k * q_k) % n
        if v == 0:
            return True
    return False


def is_bpsw_probable_prime(n: int) -> bool:
    if n < 2:
        return False
    for p in SMALL_PRIMES:
        if n == p:
            return True
        if n % p == 0:
            return False
    if gmpy2 is not None:
        try:
            return bool(gmpy2.is_bpsw_prp(int(n)))
        except Exception:
            return False
    return miller_rabin(n, [2]) and lucas_selfridge(n)


def is_probable_prime(n: int) -> bool:
    if n < 2:
        return False
    for p in SMALL_PRIMES:
        if n == p:
            return True
        if n % p == 0:
            return False
    return miller_rabin(n, [2, 3, 5, 7, 11, 13, 17])


def digits(n: int) -> int:
    return len(str(abs(n)))


def egcd(a: int, b: int) -> Tuple[int, int, int]:
    if b == 0:
        return a, 1, 0
    g, x1, y1 = egcd(b, a % b)
    return g, y1, x1 - (a // b) * y1


def crt_pair(a1: int, m1: int, a2: int, m2: int) -> Tuple[int, int]:
    g, x, _ = egcd(m1, m2)
    if (a2 - a1) % g != 0:
        raise ValueError("incompatible congruences")
    lcm = m1 // g * m2
    t = ((a2 - a1) // g * x) % (m2 // g)
    result = (a1 + m1 * t) % lcm
    return result, lcm


def build_mod_constraint(m_primes: List[int], n_primes: List[int], mod5_residue: int) -> Tuple[int, int]:
    modulus = 1
    residue = 0
    constraints = [
        (3, 8),
        (mod5_residue % 5, 5),
        (1, math.prod(m_primes) if m_primes else 1),
        ((-1) % (math.prod(n_primes) if n_primes else 1), math.prod(n_primes) if n_primes else 1),
    ]
    for a, m in constraints:
        if m == 1:
            continue
        residue, modulus = crt_pair(residue, modulus, a % m, m)
    return residue, modulus


def random_odd_in_range(rng: random.Random, low: int, high: int) -> int:
    if low % 2 == 0:
        low += 1
    if high % 2 == 0:
        high -= 1
    if low > high:
        raise ValueError("invalid range")
    return rng.randrange(low, high + 1, 2)


def find_prime_in_progression(
    rng: random.Random,
    digits_count: int,
    residue: int,
    modulus: int,
    max_steps: int,
    predicate=None,
) -> Optional[int]:
    low = 10 ** (digits_count - 1)
    high = 10 ** digits_count - 1
    if modulus <= 0:
        return None
    start = random_odd_in_range(rng, low, high)
    delta = (residue - start) % modulus
    candidate = start + delta
    if candidate < low:
        candidate += modulus
    steps = 0
    while candidate <= high and steps < max_steps:
        if candidate % 2 == 1:
            if is_probable_prime(candidate):
                if predicate is None or predicate(candidate):
                    return candidate
        candidate += modulus
        steps += 1
    return None


def find_prime_with_filters(
    rng: random.Random,
    digits_count: int,
    max_steps: int,
    predicate,
) -> Optional[int]:
    low = 10 ** (digits_count - 1)
    high = 10 ** digits_count - 1
    steps = 0
    while steps < max_steps:
        candidate = random_odd_in_range(rng, low, high)
        if predicate(candidate) and is_probable_prime(candidate):
            return candidate
        steps += 1
    return None


def generate_chernick(k: int, require_prime_factors: bool) -> Optional[Tuple[int, Dict[str, object]]]:
    f1 = 6 * k + 1
    f2 = 12 * k + 1
    f3 = 18 * k + 1
    if require_prime_factors:
        if not (is_probable_prime(f1) and is_probable_prime(f2) and is_probable_prime(f3)):
            return None
    n = f1 * f2 * f3
    meta = {
        "family": "chernick",
        "formula": "(6k+1)(12k+1)(18k+1)",
        "k": k,
        "factors": [str(f1), str(f2), str(f3)],
    }
    return n, meta


def generate_pomerance_lite(
    seed: int,
    target_digits: int,
    prime_digits: int,
    max_steps: int,
) -> Optional[Tuple[int, Dict[str, object]]]:
    rng = random.Random(seed)
    factors: List[int] = []

    def predicate(p: int) -> bool:
        return p % 8 == 3 and jacobi(5, p) == -1

    while digits(math.prod(factors) if factors else 1) < target_digits or len(factors) % 2 == 0:
        prime = find_prime_with_filters(rng, prime_digits, max_steps, predicate)
        if prime is None:
            return None
        factors.append(prime)
        if len(factors) > 9:
            break

    n = math.prod(factors)
    meta = {
        "family": "pomerance_lite",
        "prime_digits": prime_digits,
        "target_digits": target_digits,
        "factors": [str(p) for p in factors],
    }
    return n, meta


def generate_pomerance_modular(
    seed: int,
    target_digits: int,
    prime_digits: int,
    max_steps: int,
    m_primes: List[int],
    n_primes: List[int],
    mod5_residue: int,
) -> Optional[Tuple[int, Dict[str, object]]]:
    rng = random.Random(seed)
    residue, modulus = build_mod_constraint(m_primes, n_primes, mod5_residue)
    factors: List[int] = []

    while digits(math.prod(factors) if factors else 1) < target_digits or len(factors) % 2 == 0:
        prime = find_prime_in_progression(
            rng,
            prime_digits,
            residue,
            modulus,
            max_steps,
        )
        if prime is None:
            return None
        factors.append(prime)
        if len(factors) > 9:
            break

    n = math.prod(factors)
    meta = {
        "family": "pomerance_modular",
        "prime_digits": prime_digits,
        "target_digits": target_digits,
        "modulus": modulus,
        "residue": residue,
        "mod5_residue": mod5_residue,
        "m_primes": m_primes,
        "n_primes": n_primes,
        "factors": [str(p) for p in factors],
    }
    return n, meta


def parse_lambda_factors(raw: str) -> List[Tuple[int, int]]:
    if not raw:
        return DEFAULT_LAMBDA_FACTORS
    factors = []
    for entry in raw.split(","):
        entry = entry.strip()
        if not entry:
            continue
        if ":" not in entry:
            continue
        base, exp = entry.split(":", 1)
        try:
            factors.append((int(base), int(exp)))
        except ValueError:
            continue
    return factors or DEFAULT_LAMBDA_FACTORS


def generate_lambda_plus_one(
    seed: int,
    target_digits: int,
    lambda_factors: List[Tuple[int, int]],
    require_prime: bool,
    max_steps: int,
) -> Optional[Tuple[int, Dict[str, object]]]:
    rng = random.Random(seed)
    factors: List[int] = []

    while digits(math.prod(factors) if factors else 1) < target_digits or len(factors) % 2 == 0:
        attempts = 0
        while attempts < max_steps:
            d = 1
            for base, max_exp in lambda_factors:
                exp = rng.randint(0, max_exp)
                if exp:
                    d *= base**exp
            p = d + 1
            if p > 2 and (not require_prime or is_probable_prime(p)):
                factors.append(p)
                break
            attempts += 1
        if attempts >= max_steps:
            return None
        if len(factors) > 9:
            break

    n = math.prod(factors)
    meta = {
        "family": "lambda_plus_one",
        "lambda_factors": [f"{base}^{exp}" for base, exp in lambda_factors],
        "target_digits": target_digits,
        "factors": [str(p) for p in factors],
    }
    return n, meta


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--task-type",
        required=True,
        choices=[
            "main_odds",
            "large_numbers",
            "chernick",
            "pomerance_lite",
            "pomerance_modular",
            "lambda_plus_one",
        ],
    )
    parser.add_argument("--start", type=int, default=0)
    parser.add_argument("--end", type=int, default=0)
    parser.add_argument("--seed-start", type=int, default=0)
    parser.add_argument("--seed-end", type=int, default=0)
    parser.add_argument("--max-candidates", type=int, default=0)
    parser.add_argument("--target-digits", type=int, default=22)
    parser.add_argument("--prime-digits", type=int, default=7)
    parser.add_argument("--max-steps", type=int, default=5000)
    parser.add_argument("--require-prime-factors", action="store_true")
    parser.add_argument("--mod5-residue", type=int, default=2)
    parser.add_argument("--m-primes", type=str, default="")
    parser.add_argument("--n-primes", type=str, default="")
    parser.add_argument("--lambda-factors", type=str, default="")
    parser.add_argument("--require-prime", action="store_true")
    return parser.parse_args()


def parse_prime_list(raw: str, fallback: List[int]) -> List[int]:
    if not raw:
        return fallback
    values = []
    for part in raw.split(","):
        part = part.strip()
        if not part:
            continue
        try:
            values.append(int(part))
        except ValueError:
            continue
    return values or fallback


def main() -> None:
    args = parse_args()
    started = time.time()
    hits = []
    errors = []
    checked = 0

    if args.task_type in ("main_odds", "large_numbers"):
        start = args.start
        end = args.end
        if start == 0 and end == 0:
            raise SystemExit("start/end required for range tasks")
        if start % 2 == 0:
            start += 1
        for n in range(start, end + 1, 2):
            if args.max_candidates and checked >= args.max_candidates:
                break
            checked += 1
            if is_bpsw_probable_prime(n):
                hits.append({"n": str(n), "digits": digits(n), "meta": {"family": args.task_type}})

    elif args.task_type == "chernick":
        start = args.start
        end = args.end
        if start == 0 and end == 0:
            raise SystemExit("start/end required for chernick tasks")
        require_prime = args.require_prime_factors
        for k in range(start, end + 1):
            checked += 1
            candidate = generate_chernick(k, require_prime)
            if candidate is None:
                continue
            n, meta = candidate
            if is_bpsw_probable_prime(n):
                hits.append({"n": str(n), "digits": digits(n), "meta": meta})

    else:
        seed_start = args.seed_start or args.start
        seed_end = args.seed_end or args.end
        if seed_start == 0 and seed_end == 0:
            raise SystemExit("seed range required for generator tasks")
        m_primes = parse_prime_list(args.m_primes, DEFAULT_M_PRIMES)
        n_primes = parse_prime_list(args.n_primes, DEFAULT_N_PRIMES)
        lambda_factors = parse_lambda_factors(args.lambda_factors)

        for seed in range(seed_start, seed_end + 1):
            checked += 1
            if args.task_type == "pomerance_lite":
                candidate = generate_pomerance_lite(
                    seed,
                    args.target_digits,
                    args.prime_digits,
                    args.max_steps,
                )
            elif args.task_type == "pomerance_modular":
                candidate = generate_pomerance_modular(
                    seed,
                    args.target_digits,
                    args.prime_digits,
                    args.max_steps,
                    m_primes,
                    n_primes,
                    args.mod5_residue,
                )
            elif args.task_type == "lambda_plus_one":
                candidate = generate_lambda_plus_one(
                    seed,
                    args.target_digits,
                    lambda_factors,
                    args.require_prime,
                    args.max_steps,
                )
            else:
                candidate = None

            if candidate is None:
                errors.append({"seed": seed, "error": "generation_failed"})
                continue
            n, meta = candidate
            if is_bpsw_probable_prime(n):
                hits.append({"n": str(n), "digits": digits(n), "meta": meta})

    ended = time.time()
    payload = {
        "task_type": args.task_type,
        "checked": checked,
        "hit_count": len(hits),
        "hits": hits,
        "errors": errors,
        "started_at": datetime.fromtimestamp(started, tz=timezone.utc).isoformat(),
        "ended_at": datetime.fromtimestamp(ended, tz=timezone.utc).isoformat(),
    }
    print(json.dumps(payload))


if __name__ == "__main__":
    main()
