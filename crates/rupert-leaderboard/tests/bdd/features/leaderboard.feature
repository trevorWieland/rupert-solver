@leaderboard
Feature: leaderboard rendering and aggregation

  @B-0008
  Scenario: empty view still renders four sections
    Given an empty aggregated view
    When I render it
    Then the output mentions Headline, Highest clearance, Uncertified, and Open problems

  @B-0008
  Scenario: aggregation picks the best eval count per (shape, solver) pair
    Given three certified runs for cube with non-overlapping seeds
    When I aggregate them
    Then the headline has one row with best evals 50
    And the highest_clearance row has clearance 0.20

  @B-0009
  Scenario: uncertified solutions are excluded from the headline
    Given one Solved run for cube without certification
    When I aggregate them
    Then the headline is empty and uncertified has one row

  @B-0010
  Scenario: shapes with only Exhausted outcomes appear under Open problems
    Given a thousand exhausted runs for noperthedron
    When I aggregate them
    Then noperthedron is in open problems and not in the headline
