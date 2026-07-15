import { z } from 'zod'
import { TopologySchema } from '../runtime/loopal-wire'

export const HubStatusWireSchema = z.object({
  agent_count: z.number().int().nonnegative(),
  uplink: z.object({
    connected: z.boolean(),
    hub_name: z.string().min(1),
    address: z.string().nullable().optional(),
  }).nullable(),
})

export const MetaHubListWireSchema = z.object({
  hubs: z.array(z.object({
    name: z.string().min(1),
    status: z.string(),
    agent_count: z.number().int().nonnegative(),
    capabilities: z.array(z.string()).default([]),
  })),
})

export const MetaHubTopologyWireSchema = z.object({
  hubs: z.array(z.object({
    hub: z.string().min(1),
    topology: TopologySchema.or(z.object({ error: z.string() })),
  })),
})

export type MetaHubTopologyWire = z.infer<typeof MetaHubTopologyWireSchema>
