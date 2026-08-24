# NIFY – Institutional Calculation Suite (Offline, Deterministic, Rust-Powered)

![Platform](https://img.shields.io/badge/Platform-Windows%2064--bit-0078D6?style=for-the-badge)
![Offline](https://img.shields.io/badge/Offline-100%25-00C853?style=for-the-badge)
![Audit Ready](https://img.shields.io/badge/Audit--Ready-SHA--256-FF6D00?style=for-the-badge)
![Rust](https://img.shields.io/badge/Engine-Rust-DEA584?style=for-the-badge&logo=rust)
![Version](https://img.shields.io/badge/Version-v2.0.0-black?style=for-the-badge)

![NIFY Logo](./banner.png)
![NIFY Dashboard](./2banner.png)

> **An institutional-grade, offline-first calculation suite that turns 15-minute Excel workflows into <50ms deterministic results. Built for finance, engineering, and compliance teams.**

---

## 📑 Table of Contents
- [Overview](#-overview)
- [Why NIFY?](#-why-nify)
- [Performance Benchmarks](#-performance-benchmarks)
- [Tech Stack](#️-tech-stack)
- [Features & Modules](#-features--modules)
- [Verified Calculation Examples](#-verified-calculation-examples)
- [Installation & Build Instructions](#-installation--build-instructions)
- [Usage Guide](#-usage-guide)
- [Audit Trail & Compliance](#-audit-trail--compliance)
- [Project Structure](#-project-structure)
- [Troubleshooting](#-troubleshooting)
- [Contact & Professional Support](#-contact--professional-support)

---

## 🚀 Overview

**NIFY** is a next-generation desktop calculator for Windows 64-bit, designed for institutional and corporate use. It is **fully offline**, uses **no server**, and stores all data locally in a `.json` file. No SQLite or external DBMS is used.

Unlike AI/LLM tools, NIFY is **100% deterministic and AI-proof** - the same inputs will always produce the exact same output, with a verifiable SHA-256 audit hash.

**Core Philosophy:** Speed, Accuracy, and Compliance.

## 💡 Why NIFY?

| Problem in Excel / Online Tools | NIFY Solution |
| :--- | :--- |
| XIRR/NPV takes 10-15 mins with manual date handling | Calculates in **< 50ms**, one click |
| Online calculators require internet & leak financial data | **100% Offline**, data never leaves your machine |
| No audit trail, results can be manipulated | Every calculation has a **SHA-256 Hash + Timestamp + VERIFIED status** |
| Crashes with large Monte Carlo simulations | Rust-powered engine handles **500,000+ simulations** with `rayon` parallelism |

## ⚡ Performance Benchmarks

Tested on Windows 11, i5 11th Gen:

| Module | Excel / Manual Time | NIFY Time | Result |
| :--- | :--- | :--- | :--- |
| **NPV** - 6 cash flows @ 10% | ~5 Minutes | **18 ms** | $2,968.31 |
| **Loan Amortization** - 20 Years, 240 payments | ~10 Minutes | **32 ms** | EMI $4,027.97, Full Schedule |
| **XIRR** - Irregular Dates | ~15 Minutes | **41 ms** | 37.34% (Microsoft Verified) |
| **Matrix Inverse** - 3x3 | ~3 Minutes | **12 ms** | Instant |

## 🛠️ Tech Stack

This stack is MANDATORY and optimized for performance.

- **Backend Math Engine:** **Rust** – compiled as a Python extension via **PyO3 / maturin**. All heavy computations (XIRR Newton-Raphson, Monte Carlo, Matrix LU Decomposition) run in Rust. Loaded as `.pyd`.
- **Frontend/UI:** **Python 3.10+** with **PyQt6**. Professional dark theme via QSS with 3D gradient buttons. Monospace font `JetBrains Mono` for numbers.
- **Data Storage:** Local JSON file `nify_data.json` in user's home directory `%USERPROFILE%`. No SQLite.
- **Packaging:** **PyInstaller** – single `.exe` < 100 MB.
- **Libraries:** `numpy`, `openpyxl` for Excel export, `matplotlib` for embedded plots.

## 📦 Features & Modules

### 1. Financial Module
- **XIRR - Extended Internal Rate of Return:** For irregular cash flows. Correctly handles leap years (365/366 logic).
- **NPV - Net Present Value:** Supports variable time periods. Formula: `NPV = Σ Cashflow_t / (1+r)^t`
- **Loan Amortization:** Full schedule with Interest/Principal/Balance breakdown, Total Interest & Total Paid. Export to CSV.

### 2. Engineering / Scientific Module
- **Matrix Operations:** Determinant, Inverse, Transpose, Eigenvalues. Uses LU decomposition. Input format: `[[1,2],[3,4]]`

### 3. Statistics & Simulation
- **Monte Carlo Simulation:** Input Mean (e.g., `0.08` for 8%), Std Dev (e.g., `0.15`), Simulations (e.g., `10000`). Outputs Mean, Median, VaR, Percentiles + Histogram. Uses Rust `rayon` for parallel execution.
- **Linear Regression:** Input X and Y (one per line). Outputs Slope, Intercept, R², Equation `Y = mX + b` and residual plot.

### 4. Optimization
- **Simplex Method:** Linear Programming solver to maximize/minimize objective function under constraints.

### 5. Audit Trail & Compliance Log (USP)
Every calculation generates:
`SHA-256(Input + Output + Timestamp)` -> Displayed in Proof Area. Tamper-proof for corporate compliance.

## ✅ Verified Calculation Examples

Use these to verify the engine. **Input Rule: One value per line. Delete placeholder text with Ctrl+A.**

**1. NPV - Net Present Value**
```text
Discount Rate: 0.10
Cash Flows:
-50000
10000
15000
20000
15000
10000
Time Periods:
0
1
2
3
4
5
Expected Output: $2,968.31
