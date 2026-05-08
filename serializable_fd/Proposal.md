# Plumbing For First Class File Descriptor Support

## Current Challenges

- Manual de-/serialization handling for FDs
- Adding support for FDs:
  - requires additional CLI/API parameters per FD type
    - ith multiple FD parameters, associating a specific FD to the list of FDs supplied by SCM_RIGHTS gets complex
  - 

## Scenarios

- Passing a list of FDs to CHV that are associated to a single entity, for example multiple FDs for a single net device.
- Passing a list of FDs to CHV that are associated with various entities, for example net devices, disks, etc..

## Open Questions

- What to do with the D-BUS API?
  - As far as I can tell, there is no way to transfer FDs via D-BUS, so FD support is a non-starter.
- 