@cli
Feature: rupert CLI surface

  @B-0002
  Scenario: rupert list solvers names every registered solver
    Given the rupert CLI is built
    When I run "rupert list solvers"
    Then the exit code is 0
    And stdout contains "random_quat"
    And stdout contains "face_normal_pairs"
    And stdout contains "nelder_mead"
    And stdout contains "random_then_refine"
    And stdout contains "hopf_grid"

  @B-0002
  Scenario: rupert list shapes names every builtin
    Given the rupert CLI is built
    When I run "rupert list shapes"
    Then the exit code is 0
    And stdout contains "cube"
    And stdout contains "tetrahedron"
    And stdout contains "octahedron"
    And stdout contains "dodecahedron"
    And stdout contains "icosahedron"
    And stdout contains "pentagonal_icositetrahedron"
    And stdout contains "snub_cube"
    And stdout contains "noperthedron"

  @B-0001
  Scenario: rupert run produces a certified solution for cube + random_quat
    Given the rupert CLI is built
    And a fresh working directory
    When I run "rupert run --shape cube --solver random_quat --seed 0 --budget-evals 10000"
    Then the exit code is 0
    And a result file exists with at least one Solved certified record
    And a result file uses schema v2 observation classes

  @B-0005
  Scenario: rupert analyze summary reports run outcomes
    Given the rupert CLI is built
    And a fresh working directory
    When I run "rupert run --shape cube --solver random_quat --seed 0 --budget-evals 10000"
    Then the exit code is 0
    When I run "rupert analyze summary --results-dir results"
    Then the exit code is 0
    And stdout contains "solved"
    And stdout contains "best_positive"

  @B-0006
  Scenario: rupert analyze patch-aware reports top cells
    Given the rupert CLI is built
    And a fresh working directory
    And a fabricated patch-aware result on disk
    When I run "rupert analyze patch-aware --results-dir results --shape snub_cube --top 1"
    Then the exit code is 0
    And stdout contains "patch_aware"
    And stdout contains "near_miss"

  @B-0003
  Scenario: rupert run with unknown solver fails with a clear message
    Given the rupert CLI is built
    And a fresh working directory
    When I run "rupert run --shape cube --solver does_not_exist --seed 0 --budget-evals 100"
    Then the exit code is non-zero
    And stderr contains "does_not_exist"

  @B-0004
  Scenario: rupert verify rejects a fabricated noperthedron passage
    Given the rupert CLI is built
    And a fresh working directory
    And a fabricated noperthedron-passage result on disk
    When I run "rupert verify results"
    Then the exit code is 0
    And the fabricated record's outcome is now Disqualified
