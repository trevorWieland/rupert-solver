@bench
Feature: rupert-bench single-cell runner

  @B-0006
  Scenario: budget exhaustion produces Exhausted, not Error
    Given a cube polyhedron
    When FaceNormalPairs runs against the cube with budget 5
    Then the outcome is Exhausted

  @B-0001
  Scenario: cube run with FaceNormalPairs produces a certified solution
    Given a cube polyhedron
    When FaceNormalPairs runs against the cube with budget 110000
    Then the outcome is Solved
    And the result has a certified solution

  @B-0007
  Scenario: parallel sweep with fixed seeds is deterministic across runs
    When I run a sweep over cube/octahedron with seeds 0,1,2 twice
    Then both sweeps yield byte-equal solution payloads
