<script lang="ts">
  import type { DeviceInfo } from '#lib/generated/api.ts';
  import DeviceRow from './DeviceRow.svelte';
  let { devices, onrevoke }: { devices: DeviceInfo[]; onrevoke: (device: DeviceInfo) => void } = $props();
</script>
<ul class="panel divide-y divide-separator">{#each devices as device (device.id)}<DeviceRow {device} {onrevoke} />{/each}</ul>
