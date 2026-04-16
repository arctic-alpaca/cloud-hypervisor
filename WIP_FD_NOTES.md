# Notes for WIP FD support

## Use Cases

- Live migration
  - ...
- Live update for CHV
  - ...
- VM creation
  - ...
- Add net 
- Add blk
- Exporting FDs for safekeeping during updates/migrations/???

## Things that are FDs

- serial
- console
- disk
- fs
- pmem
- (vfio-)user-device
- (vfio-)device
- vsock
- vdpa
- virtio-device-backend
- memory
- vCPUs
- VM
- vhost-user-{net,block,fs,...}
- files
  - api socket
  - event monitor
  - 


## Where do FDs come from?

AFAICT, there is no way to validate the user provided input.

- Passed in via command line
  `fds=[1,2,3]`
  - FDs are created from the given indices
  - The passed in values must match the passed in FDs
  - the indices must have attached metadata to make them identifiable to associate them with the correct device and id
  - the FDs passed in must also have a 
- HTTP API but supplied via `SCM_RIGHTS`
  - FDs are created from a separate `SCM_RIGHTS` message that needs to be correlated with the API request
  - The `SCM_RIGHTS` message must have the same order of FDs as the request body
  - The request body must contain FD metadata in the same order as the `SCM_RIGHTS` message
- DBUS API, supplied via string again (same as CLI)
  - How can they be added to the fd-table of the chv process?

## FD metadata

- device type
- device id/index?

## Requirements

- FDs must be typed and only usable for their intended purpose
- FDs can be passed to CHV via API and CLI
- Extract FDs from CHV again
  - For example for live migration or update
- It should be obvious whether FDs need to be corrected or created from ints
- We need to be able to send all FDs in a single API call, otherwise `restore` would need multiple `SCM_RIGHTS` messages

### List

- [ ] Get FDs of any kind into CHV
  - [ ] Ingest FDs from CLI
  - [ ] Ingest FDs from HTTP API
- [ ] apply FDs of any kind to the VM

Optional:
- [ ] Extract FDs from CHV 
  - [ ] Export FDs to HTTP API

## Implementation

- Type all FD
  - first combine all FD inputs to use types FDs to show design
- The HTTP API should return a fully assembled config, can we do that?
  - No, the API does not have access to the `VM` struct
- Parsing the CLI input should result in a fully assembled config
- CLI notation for FDs: `fds=[net(id)@[1,2,4,5],disk(id)@[6,7],net(id)@[8],...]`
  - can be achieved by `Tuple<Device, Vec<u64>>`
  - can be mixed in restore
  - only net can be used in `AddNet`, only blk in `AddBlk` and so on
  - The id is required to identify the correct device/?? if there are multiple
    - Is this needed for all things that use FDs?


## Resources

- https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/macvtap-bridge.md
- https://github.com/cloud-hypervisor/cloud-hypervisor/issues/6286
- https://github.com/cloud-hypervisor/cloud-hypervisor/issues/7704
- 