import { render } from 'solid-js/web'
import { afterEach, describe, expect, it, vi } from 'vitest'
import App from './App'

const todo = {
  id: 'todo-1',
  title: 'Learn SolidJS',
  description: 'Reactive TODO',
  completed: false,
  created_at: '2026-10-05T00:00:00Z',
  updated_at: '2026-10-05T00:00:00Z',
}

let dispose: (() => void) | undefined

function mount() {
  const root = document.createElement('div')
  document.body.append(root)
  dispose = render(() => <App />, root)
  return root
}

function button(root: HTMLElement, label: string) {
  const element = [...root.querySelectorAll('button')].find(button => button.textContent?.trim() === label)
  if (!element) throw new Error(`Button not found: ${label}`)
  return element
}

function input(element: HTMLInputElement, value: string) {
  element.value = value
  element.dispatchEvent(new Event('input', { bubbles: true }))
}

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } })
}

afterEach(() => {
  dispose?.()
  dispose = undefined
  document.body.replaceChildren()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('SolidJS TODO app', () => {
  it('loads, creates, edits, cancels, toggles, refreshes and deletes todos', async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce(json([]))
    vi.stubGlobal('fetch', fetchMock)
    const root = mount()
    expect(root.textContent).toContain('Loading...')
    await vi.waitFor(() => expect(root.textContent).toContain('No todos yet'))

    button(root, 'Add TODO').click()
    expect(fetchMock).toHaveBeenCalledTimes(1)
    input(root.querySelector<HTMLInputElement>('[placeholder="Todo title"]')!, todo.title)
    input(root.querySelector<HTMLInputElement>('[placeholder="Description (optional)"]')!, todo.description)
    fetchMock.mockResolvedValueOnce(json(todo, 201)).mockResolvedValueOnce(json([todo]))
    button(root, 'Add TODO').click()
    await vi.waitFor(() => expect(root.querySelector('h3')?.textContent).toContain(todo.title))
    expect(fetchMock).toHaveBeenNthCalledWith(2, '/api/todos', expect.objectContaining({
      method: 'POST', body: JSON.stringify({ title: todo.title, description: todo.description }),
    }))
    expect(root.querySelector<HTMLInputElement>('[placeholder="Todo title"]')?.value).toBe('')

    button(root, 'Edit').click()
    input(root.querySelector<HTMLInputElement>('input:not([placeholder])')!, 'Discard this')
    button(root, 'Cancel').click()
    expect(root.querySelector('h3')?.textContent).toContain(todo.title)
    expect(fetchMock).toHaveBeenCalledTimes(3)

    button(root, 'Edit').click()
    input(root.querySelector<HTMLInputElement>('input:not([placeholder])')!, 'Updated title')
    input(root.querySelectorAll<HTMLInputElement>('input:not([placeholder])')[1], 'Updated description')
    const updated = { ...todo, title: 'Updated title', description: 'Updated description' }
    fetchMock.mockResolvedValueOnce(json(updated)).mockResolvedValueOnce(json([updated]))
    button(root, 'Save').click()
    await vi.waitFor(() => expect(root.querySelector('h3')?.textContent).toContain(updated.title))
    expect(fetchMock).toHaveBeenNthCalledWith(4, '/api/todos/todo-1', expect.objectContaining({
      method: 'PUT', body: JSON.stringify({ title: updated.title, description: updated.description }),
    }))

    const completed = { ...updated, completed: true }
    fetchMock.mockResolvedValueOnce(json(completed)).mockResolvedValueOnce(json([completed]))
    button(root, 'Mark Complete').click()
    await vi.waitFor(() => expect(button(root, 'Mark Incomplete')).toBeDefined())
    expect(fetchMock).toHaveBeenNthCalledWith(6, '/api/todos/todo-1', expect.objectContaining({
      method: 'PUT', body: JSON.stringify({ completed: true }),
    }))
    expect(root.querySelector('h3')?.style.textDecoration).toBe('line-through')

    fetchMock.mockResolvedValueOnce(json([completed]))
    button(root, 'Refresh TODOs').click()
    await vi.waitFor(() => expect(root.querySelector('h3')?.textContent).toContain(updated.title))
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 })).mockResolvedValueOnce(json([]))
    button(root, 'Delete').click()
    await vi.waitFor(() => expect(root.textContent).toContain('No todos yet'))
    expect(fetchMock).toHaveBeenNthCalledWith(9, '/api/todos/todo-1', { method: 'DELETE' })
  })

  it('aborts the initial request when the component is disposed', () => {
    const fetchMock = vi.fn<typeof fetch>(() => new Promise<Response>(() => {}))
    vi.stubGlobal('fetch', fetchMock)
    mount()
    const signal = fetchMock.mock.calls[0][1]!.signal!
    expect(signal.aborted).toBe(false)
    dispose!()
    dispose = undefined
    expect(signal.aborted).toBe(true)
  })

  it('stops showing loading when the initial request fails', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('Network unavailable')))
    const error = vi.spyOn(console, 'error').mockImplementation(() => {})
    const root = mount()
    await vi.waitFor(() => expect(root.textContent).toContain('No todos yet'))
    expect(root.textContent).not.toContain('Loading...')
    expect(error).toHaveBeenCalledWith('Failed to fetch todos:', expect.any(Error))
  })
})
