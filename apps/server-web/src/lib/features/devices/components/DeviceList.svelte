<script lang="ts">
  import type { DeviceInfo } from '#lib/generated/api.ts';
  import DeviceRow from './DeviceRow.svelte';
  let { devices, onrevoke, ondelete }: { devices: DeviceInfo[]; onrevoke: (device: DeviceInfo) => void; ondelete: (device: DeviceInfo) => void } = $props();
</script>
<ul class="panel divide-y divide-separator">{#each devices as device (device.id)}<DeviceRow {device} {onrevoke} {ondelete} />{/each}</ul>
