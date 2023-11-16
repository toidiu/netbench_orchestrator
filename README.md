# Netbench Orchestrator

Used to run netbench scenarios at scale.

## Goals
Often developers of transport protocols are interested in gather performance data for the protocol
they are developing. Netbench is a tool which can be used to measure this performance data.
However, in-order to get useful results its often necessary to run Netbench scenarios in the cloud
so that the results better match production systems. The goal of this project is to automate
Netbench runs in the cloud.

## Implementation details

### Russula
Russula is a synchronization framework that exposes a Coordinator to Workers relationship. The


