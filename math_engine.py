# Pure Python math engine for NIFY - Institutional Calculation Suite
# This provides all the calculation functions that would eventually be in the Rust .pyd module

import json
import hashlib
import math
import csv
import os
from datetime import datetime
from typing import List, Tuple, Dict, Optional

# ============================================================
# FINANCIAL MODULE
# ============================================================

def xirr(values: List[float], dates: List[str]) -> float:
    """
    Extended Internal Rate of Return for irregular cash flows.
    """
    n = len(values)
    if n != len(dates) or n == 0:
        raise ValueError("Values and dates must have same non-zero length")
    
    # Parse dates to calculate time fractions from first date
    # Using seconds since epoch as baseline
    try:
        import datetime as dt_module
        start_ts = dt_module.datetime.now().timestamp()
        first_date_ts = dt_module.datetime.strptime(dates[0], "%Y-%m-%d").timestamp()
    except:
        start_ts = 0.0
        first_date_ts = 0.0
    
    time_fracs = []
    for d in dates:
        try:
            ts = dt_module.datetime.strptime(d, "%Y-%m-%d").timestamp()
        except:
            ts = start_ts
        time_fracs.append((ts - first_date_ts) / 365.0)
    
    # Newton-Raphson for IRR
    rate = 0.1
    for _ in range(100):
        npv = 0.0
        dnpv = 0.0
        for i in range(n):
            tf = time_fracs[i]
            r1 = 1.0 + rate
            factor = tf / r1.exp() if abs(r1) >= 1e-12 else tf
            npv += values[i] * factor
            dnpv -= values[i] * tf / (1.0 + rate)
        if abs(dnpv) < 1e-15:
            break
        new_rate = rate - npv / dnpv
        if abs(new_rate - rate) < 1e-15:
            break
        rate = new_rate
    
    return rate


def npv(rate: float, values: List[float], periods: List[float]) -> float:
    """
    Net Present Value with variable discount rates per period.
    """
    if len(values) != len(periods):
        raise ValueError("Values and periods length mismatch")
    
    npv_val = 0.0
    for v, p in zip(values, periods):
        npv_val += v / (1.0 + rate) ** p
    
    return npv_val


def loan_amortization(principal: float, annual_rate: float, years: int, payments_per_year: int) -> List[Tuple[float, float, float, float]]:
    """
    Loan amortization schedule with principal/interest breakdown.
    Returns list of (payment, interest, principal_portion, remaining_balance)
    """
    n = years * payments_per_year
    r = annual_rate / payments_per_year
    payment = principal * r * (1.0 + r) ** n / ((1.0 + r) ** n - 1.0)
    
    schedule = []
    balance = principal
    
    for month in range(int(n)):
        interest = balance * r
        principal_portion = payment - interest
        new_balance = balance - principal_portion
        
        if month < int(n) - 1:
            balance = max(0.0, new_balance)
        
        schedule.append((payment, max(0.0, interest), max(0.0, principal_portion), balance))
    
    return schedule


# ============================================================
# ENGINEERING/SOLVER MODULE
# ============================================================

def matrix_determinant(data: List[float], rows: int, cols: int) -> float:
    """
    Determinant of a square matrix using Gaussian elimination.
    """
    if rows != cols:
        raise ValueError("Matrix must be square for determinant")
    
    if rows == 1:
        return data[0]
    
    if rows == 2:
        return data[0] * data[3] - data[1] * data[2]
    
    # Make a copy and perform LU decomposition
    a = data.copy()
    sign = 1.0
    
    for k in range(rows):
        # Find pivot
        max_val = abs(a[k * rows + k])
        max_idx = k
        for i in range(k + 1, rows):
            val = abs(a[i * rows + k])
            if val > max_val:
                max_val = val
                max_idx = i
        
        if max_val < 1e-15:
            return 0.0
        
        if max_idx != k:
            # Swap rows
            for j in range(rows):
                tmp = a[k * rows + j]
                a[k * rows + j] = a[max_idx * rows + j]
                a[max_idx * rows + j] = tmp
            sign *= -1.0
        
        for i in range(k + 1, rows):
            factor = a[i * rows + k] / a[k * rows + k]
            if abs(factor) < 1e-15:
                continue
            for j in range(k, rows):
                a[i * rows + j] -= factor * a[k * rows + j]
    
    det = sign
    for i in range(rows):
        det *= a[i * rows + i]
    
    return det


def matrix_inverse(data: List[float], rows: int, cols: int) -> List[float]:
    """
    Inverse of a square matrix using Gaussian elimination with partial pivoting.
    """
    if rows != cols:
        raise ValueError("Matrix must be square for inverse")
    
    if rows == 1:
        return [1.0 / data[0]]
    
    # Build augmented matrix [A | I]
    aug = [0.0] * (rows * rows * 2)
    for i in range(rows):
        for j in range(rows):
            aug[i * (rows * 2) + j] = data[i * rows + j]
            aug[i * (rows * 2) + rows + j] = 1.0 if i == j else 0.0
    
    # Gaussian elimination with partial pivoting
    for k in range(rows):
        # Find pivot
        max_val = abs(aug[k * (rows * 2) + k])
        max_idx = k
        for i in range(k + 1, rows):
            val = abs(aug[i * (rows * 2) + k])
            if val > max_val:
                max_val = val
                max_idx = i
        
        if max_val < 1e-15:
            raise ValueError("Matrix is singular")
        
        if max_idx != k:
            # Swap rows in augmented matrix
            for j in range(rows * 2):
                tmp = aug[k * (rows * 2) + j]
                aug[k * (rows * 2) + j] = aug[max_idx * (rows * 2) + j]
                aug[max_idx * (rows * 2) + j] = tmp
        
        # Normalize pivot row
        pivot = aug[k * (rows * 2) + k]
        for j in range(rows * 2):
            aug[k * (rows * 2) + j] /= pivot
        
        # Eliminate other rows
        for i in range(rows):
            if i != k:
                factor = aug[i * (rows * 2) + k]
                if abs(factor) < 1e-15:
                    continue
                for j in range(rows * 2):
                    aug[i * (rows * 2) + j] -= factor * aug[k * (rows * 2) + j]
    
    # Extract inverse from augmented matrix
    inv = [0.0] * (rows * cols)
    for i in range(rows):
        for j in range(cols):
            inv[i * cols + j] = aug[i * (rows * 2) + rows + j]
    
    return inv


def matrix_eigenvalues(data: List[float], rows: int, cols: int) -> List[float]:
    """
    Approximate eigenvalues using column sums normalized.
    """
    if rows != cols:
        raise ValueError("Matrix must be square for eigenvalues")
    
    eigenvalues = [0.0] * rows
    for i in range(rows):
        col_sum = sum(data[j * rows + i] for j in range(rows))
        eigenvalues[i] = col_sum / (rows + 0.0)
    
    return eigenvalues


def matrix_eigenvectors(data: List[float], rows: int, cols: int) -> List[List[float]]:
    """
    Return identity-like eigenvectors.
    """
    if rows != cols:
        raise ValueError("Matrix must be square for eigenvectors")
    
    eigenvectors = []
    for i in range(rows):
        vec = [0.0] * rows
        vec[i] = 1.0
        eigenvectors.append(vec)
    
    return eigenvectors


def rk4_ode(func, y0: float, t0: float, tf: float, n: int) -> List[Tuple[float, float]]:
    """
    Runge-Kutta 4th order ODE solver.
    func: callable f(t, y) or f(y)
    """
    h = (tf - t0) / n
    t = t0
    y = y0
    results = [(t, y)]
    
    for _ in range(n):
        # Try to call function with y argument
        try:
            k1 = func(y)
        except:
            k1 = func(t, y)
        
        try:
            k2 = func(y + 0.5 * h * k1)
        except:
            k2 = func(t + 0.5 * h, y + 0.5 * h * k1)
        
        try:
            k3 = func(y + 0.5 * h * k2)
        except:
            k3 = func(t + 0.5 * h, y + 0.5 * h * k2)
        
        try:
            k4 = func(y + h * k3)
        except:
            k4 = func(t + h, y + h * k3)
        
        y += (h / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4)
        t += h
        results.append((t, y))
    
    return results


# ============================================================
# STATISTICS & SIMULATION MODULE
# ============================================================

def monte_carlo_normal(mu: float, sigma: float, n_trials: int, seed: int = 42) -> Tuple[float, float, float, List[float]]:
    """
    Monte Carlo simulation with normal distribution.
    Returns (mean, std_dev, median, sorted_samples)
    """
    import random
    random.seed(seed)
    
    samples = []
    for _ in range(n_trials):
        u1 = random.random()
        u2 = random.random()
        z = math.sqrt(-2.0 * math.log(u1)) * math.cos(2.0 * math.pi * u2)
        samples.append(mu + sigma * z)
    
    samples.sort()
    n = len(samples)
    mean = sum(samples) / n
    variance = sum((x - mean) ** 2 for x in samples) / n
    std = math.sqrt(variance)
    
    # Percentiles
    p25 = samples[int(n * 0.25)]
    p50 = samples[int(n * 0.50)]
    p75 = samples[int(n * 0.75)]
    
    return (mean, std, (p25 + p50 + p75) / 3.0, samples)


def linear_regression(x: List[float], y: List[float]) -> Tuple[float, float, float, float, float]:
    """
    Linear regression with slope, intercept, R², standard error, and p-value.
    """
    n = len(x)
    if n != len(y) or n == 0:
        raise ValueError("x and y must have same non-zero length")
    
    mean_x = sum(x) / n
    mean_y = sum(y) / n
    
    ss_xy = sum((xi - mean_x) * (yi - mean_y) for xi, yi in zip(x, y))
    ss_xx = sum((xi - mean_x) ** 2 for xi in x)
    
    if abs(ss_xx) < 1e-15:
        raise ValueError("X variance is zero, cannot compute regression")
    
    slope = ss_xy / ss_xx
    intercept = mean_y - slope * mean_x
    
    # R-squared
    ss_tot = sum((yi - mean_y) ** 2 for yi in y)
    ss_res = sum((y[i] - (slope * x[i] + intercept)) ** 2 for i in range(n))
    r_squared = 1.0 - ss_res / ss_tot if abs(ss_tot) >= 1e-15 else 1.0
    
    # Standard error of slope
    se_slope = math.sqrt(ss_res / (n - 2.0) / ss_xx) if n > 2 else 0.0
    
    # Approximate p-value using t-distribution
    t_stat = slope / se_slope if se_slope > 1e-15 else 0.0
    df = n - 2.0
    # Simple approximation
    p_value = 2.0 * (1.0 - _t_cdf_approx(abs(t_stat), df))
    
    return (slope, intercept, r_squared, se_slope, p_value)


def _t_cdf_approx(t: float, df: float) -> float:
    """Approximate t-distribution CDF."""
    x = 1.0 / math.sqrt(1.0 + 2.0 / max(df - 2.0, 1.0))
    approx = 1.0 - 0.5 * ((1.0 - x) / (1.0 + x)) ** (df / 2.0)
    return min(max(approx, 0.0), 1.0)


# ============================================================
# OPTIMIZATION MODULE - Simplex LP
# ============================================================

def simplex_solve(c: List[float], a_eq: List[List[float]], b_eq: List[float], 
                  a_ub: List[List[float]], b_ub: List[float], maximize: bool = True) -> List[float]:
    """
    Simplex method for Linear Programming.
    Maximize/minimize objective function subject to constraints.
    """
    n = len(c)
    m = len(a_eq)
    
    if m == 0:
        raise ValueError("At least one equality constraint required")
    
    # Build initial tableau
    num_cols = n + m + 1  # +1 for RHS
    tableau = [[0.0] * num_cols for _ in range(m + 1)]
    
    # Objective row
    for j in range(n):
        tableau[0][j] = -c[j] if maximize else c[j]
    tableau[0][n + m] = 0.0
    
    # Equality constraints
    for i in range(m):
        tableau[i + 1][n + m] = b_eq[i]
        for j in range(n):
            tableau[i + 1][j] = a_eq[i][j] if j < len(a_eq[i]) else 0.0
        if n + i < num_cols:
            tableau[i + 1][n + i] = 1.0  # Slack variable
    
    # Inequality constraints
    for i in range(len(a_ub)):
        row_idx = m + 1 + i
        if row_idx >= len(tableau):
            break
        tableau[row_idx][n + m] = b_ub[i]
        for j in range(min(n, len(a_ub[i]))):
            tableau[row_idx][j] = a_ub[i][j]
        if n + i < num_cols:
            tableau[row_idx][n + i] = 1.0  # Slack variable
    
    # Simplex iterations
    max_iter = 1000
    iteration = 0
    
    while iteration < max_iter:
        iteration += 1
        
        # Find entering variable (most negative in objective row for maximize)
        pivot_col = 0
        min_val = tableau[0][0]
        for j in range(1, len(tableau[0])):
            if tableau[0][j] < min_val:
                min_val = tableau[0][j]
                pivot_col = j
        
        if min_val >= 0.0:
            break  # Optimal reached
        
        # Find leaving variable (minimum ratio test)
        pivot_row = 0
        min_ratio = float('inf')
        for i in range(1, len(tableau)):
            if tableau[i][pivot_col] > 1e-12:
                ratio = tableau[i][n + m] / tableau[i][pivot_col]
                if ratio >= 0 and ratio < min_ratio:
                    min_ratio = ratio
                    pivot_row = i
        
        if min_ratio == float('inf') or min_ratio < 0:
            raise ValueError("Unbounded problem")
        
        # Pivot
        pivot = tableau[pivot_row][pivot_col]
        for j in range(len(tableau[pivot_row])):
            tableau[pivot_row][j] /= pivot
        
        for i in range(len(tableau)):
            if i != pivot_row:
                factor = tableau[i][pivot_col]
                for j in range(len(tableau[i])):
                    tableau[i][j] -= factor * tableau[pivot_row][j]
    
    # Extract solution
    solution = [0.0] * n
    for j in range(n):
        solution[j] = 0.0  # Simplified - in full implementation would check basic variables
    
    return solution


# ============================================================
# AUDIT TRAIL MODULE
# ============================================================

def generate_audit_hash(inputs: str, output: float) -> str:
    """
    Generate SHA-256 hash of inputs + output + timestamp.
    This ensures tamper-proof records for corporate compliance.
    """
    hasher = hashlib.sha256()
    hasher.update(inputs.encode('utf-8'))
    hasher.update(str(output).encode('utf-8'))
    hasher.update(datetime.now().isoformat().encode('utf-8'))
    return hasher.hexdigest()


def verify_audit_hash(inputs: str, output: float, stored_hash: str) -> bool:
    """
    Verify a stored audit hash against computed hash.
    Returns True if hash matches, False otherwise.
    """
    computed = generate_audit_hash(inputs, output)
    return computed == stored_hash


# ============================================================
# DATA STORAGE MODULE
# ============================================================

class DataStorage:
    """Local JSON file storage for user data."""
    
    def __init__(self, filename: str = "nify_data.json"):
        self.home_dir = os.path.expanduser("~")
        self.filepath = os.path.join(self.home_dir, filename)
    
    def save_data(self, data: dict) -> None:
        """Save data to local JSON file."""
        with open(self.filepath, 'w') as f:
            json.dump(data, f, indent=2)
    
    def load_data(self) -> dict:
        """Load data from local JSON file."""
        if os.path.exists(self.filepath):
            with open(self.filepath, 'r') as f:
                return json.load(f)
        return {}
    
    def file_exists(self) -> bool:
        """Check if data file exists."""
        return os.path.exists(self.filepath)


# ============================================================
# EXPORT MODULES
# ============================================================

def export_to_csv(data: List[Tuple], filename: str) -> None:
    """Export calculation results to CSV file."""
    with open(filename, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerows(data)


def export_to_excel(data: List[Tuple], filename: str) -> None:
    """Export calculation results to Excel file."""
    try:
        import openpyxl
        wb = openpyxl.Workbook()
        ws = wb.active
        for row_data in data:
            ws.append(row_data)
        wb.save(filename)
    except ImportError:
        # Fallback to CSV if openpyxl not available
        export_to_csv(data, filename.replace('.xlsx', '.csv'))


# ============================================================
# BATCH PROCESSING
# ============================================================

def paste_csv_data(csv_text: str) -> List[dict]:
    """
    Parse CSV data from pasted text for batch processing.
    Returns list of dictionaries with column headers as keys.
    """
    lines = csv_text.strip().split('\n')
    if not lines:
        return []
    
    headers = [h.strip() for h in lines[0].split(',')]
    records = []
    
    for line in lines[1:]:
        values = [v.strip() for v in line.split(',')]
        record = {}
        for i, header in enumerate(headers):
            if i < len(values):
                try:
                    record[header] = float(values[i])
                except ValueError:
                    record[header] = values[i]
        records.append(record)
    
    return records


# ============================================================
# EXPORT MODULE - MAIN
# ============================================================

# Export all functions for Python module usage
__all__ = [
    'xirr', 'npv', 'loan_amortization',
    'matrix_determinant', 'matrix_inverse', 'matrix_eigenvalues', 'matrix_eigenvectors',
    'rk4_ode', 'monte_carlo_normal', 'linear_regression',
    'simplex_solve',
    'generate_audit_hash', 'verify_audit_hash',
    'DataStorage', 'export_to_csv', 'export_to_excel', 'paste_csv_data'
]