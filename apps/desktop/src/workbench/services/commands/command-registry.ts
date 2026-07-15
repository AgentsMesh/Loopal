import { type IDisposable, toDisposable } from '../../../base/common/lifecycle'

export interface CommandContext {
  readonly source: 'menu' | 'keybinding' | 'palette' | 'api'
}

export interface CommandDescriptor {
  readonly id: string
  readonly title: string
  readonly category?: string
  readonly keybinding?: string
}

interface CommandEntry extends CommandDescriptor {
  readonly handler: (context: CommandContext, ...args: unknown[]) => unknown
}

export class CommandRegistry {
  private readonly commands = new Map<string, CommandEntry>()

  register(
    descriptor: CommandDescriptor,
    handler: (context: CommandContext, ...args: unknown[]) => unknown,
  ): IDisposable {
    if (this.commands.has(descriptor.id)) {
      throw new Error(`Command already registered: ${descriptor.id}`)
    }
    const entry: CommandEntry = { ...descriptor, handler }
    this.commands.set(descriptor.id, entry)
    return toDisposable(() => {
      this.commands.delete(descriptor.id)
    })
  }

  list(): readonly CommandDescriptor[] {
    return [...this.commands.values()]
      .map(({ handler: _handler, ...descriptor }) => descriptor)
      .sort((left, right) => left.title.localeCompare(right.title))
  }

  execute(id: string, context: CommandContext, ...args: unknown[]): unknown {
    const command = this.commands.get(id)
    if (!command) {
      throw new Error(`Command not found: ${id}`)
    }
    return command.handler(context, ...args)
  }
}
