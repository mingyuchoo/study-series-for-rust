import { createSignal, For, onCleanup, onMount } from 'solid-js'
import './App.css'

interface Todo {
  id: string;
  title: string;
  description?: string;
  completed: boolean;
  created_at: string;
  updated_at: string;
}

interface CreateTodo {
  title: string;
  description?: string;
}

function App() {
  const [todos, setTodos] = createSignal<Todo[]>([])
  const [loading, setLoading] = createSignal(true)
  const [newTodo, setNewTodo] = createSignal<CreateTodo>({ title: '', description: '' })
  const [editingTodo, setEditingTodo] = createSignal<Todo | null>(null)

  const fetchTodos = async () => {
    setLoading(true)
    try {
      const response = await fetch('/api/todos')
      const data = await response.json()
      setTodos(data)
    } catch (error) {
      console.error('Failed to fetch todos:', error)
    } finally {
      setLoading(false)
    }
  }

  const createTodo = async () => {
    if (!newTodo().title.trim()) return

    try {
      const response = await fetch('/api/todos', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(newTodo()),
      })

      if (response.ok) {
        setNewTodo({ title: '', description: '' })
        fetchTodos()
      }
    } catch (error) {
      console.error('Failed to create todo:', error)
    }
  }

  const updateTodo = async (id: string, updates: Partial<Todo>) => {
    try {
      const response = await fetch(`/api/todos/${id}`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(updates),
      })

      if (response.ok) {
        fetchTodos()
        setEditingTodo(null)
      }
    } catch (error) {
      console.error('Failed to update todo:', error)
    }
  }

  const deleteTodo = async (id: string) => {
    try {
      const response = await fetch(`/api/todos/${id}`, {
        method: 'DELETE',
      })

      if (response.ok) {
        fetchTodos()
      }
    } catch (error) {
      console.error('Failed to delete todo:', error)
    }
  }

  const toggleComplete = (todo: Todo) => {
    updateTodo(todo.id, { completed: !todo.completed })
  }

  onMount(() => {
    const controller = new AbortController()
    fetch('/api/todos', { signal: controller.signal })
      .then(response => response.json())
      .then(data => {
        if (!controller.signal.aborted) setTodos(data)
      })
      .catch(error => {
        if (!controller.signal.aborted) console.error('Failed to fetch todos:', error)
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    onCleanup(() => controller.abort())
  })

  return (
    <>
      <div>
        <a href="https://vite.dev" target="_blank">
          <img src="/vite.svg" class="logo" alt="Vite logo" />
        </a>
        <a href="https://www.solidjs.com" target="_blank">
          <span class="solid-brand">SolidJS</span>
        </a>
      </div>
      <h1>TODO Manager</h1>

      <div class="card">
        <h2>Add New TODO</h2>
        <div style={{ 'margin-bottom': '1rem' }}>
          <input
            type="text"
            placeholder="Todo title"
            value={newTodo().title}
            onInput={(e) => setNewTodo({ ...newTodo(), title: e.currentTarget.value })}
            style={{
              'margin-right': '0.5rem',
              padding: '0.5rem',
              'background-color': '#1a1a1a',
              border: '1px solid #404040',
              'border-radius': '4px',
              color: 'rgba(255, 255, 255, 0.87)'
            }}
          />
          <input
            type="text"
            placeholder="Description (optional)"
            value={newTodo().description}
            onInput={(e) => setNewTodo({ ...newTodo(), description: e.currentTarget.value })}
            style={{
              'margin-right': '0.5rem',
              padding: '0.5rem',
              'background-color': '#1a1a1a',
              border: '1px solid #404040',
              'border-radius': '4px',
              color: 'rgba(255, 255, 255, 0.87)'
            }}
          />
          <button onClick={createTodo}>Add TODO</button>
        </div>
      </div>

      <div class="card">
        <h2>TODO List</h2>
        {loading() ? (
          <p>Loading...</p>
        ) : todos().length > 0 ? (
          <div>
            <For each={todos()}>{todo => (
              <div style={{
                border: '1px solid #404040',
                margin: '0.5rem 0',
                padding: '1rem',
                'border-radius': '8px',
                'background-color': todo.completed ? '#1a2e1a' : '#2a2a2a',
                'box-shadow': '0 2px 4px rgba(0, 0, 0, 0.3)'
              }}>
                {editingTodo()?.id === todo.id ? (
                  <div>
                    <input
                      type="text"
                      value={editingTodo()!.title}
                      onInput={(e) => setEditingTodo({ ...editingTodo()!, title: e.currentTarget.value })}
                      style={{
                        'margin-right': '0.5rem',
                        padding: '0.5rem',
                        'background-color': '#1a1a1a',
                        border: '1px solid #404040',
                        'border-radius': '4px',
                        color: 'rgba(255, 255, 255, 0.87)'
                      }}
                    />
                    <input
                      type="text"
                      value={editingTodo()!.description || ''}
                      onInput={(e) => setEditingTodo({ ...editingTodo()!, description: e.currentTarget.value })}
                      style={{
                        'margin-right': '0.5rem',
                        padding: '0.5rem',
                        'background-color': '#1a1a1a',
                        border: '1px solid #404040',
                        'border-radius': '4px',
                        color: 'rgba(255, 255, 255, 0.87)'
                      }}
                    />
                    <button onClick={() => updateTodo(todo.id, {
                      title: editingTodo()!.title,
                      description: editingTodo()!.description
                    })}>
                      Save
                    </button>
                    <button onClick={() => setEditingTodo(null)} style={{ 'margin-left': '0.5rem' }}>
                      Cancel
                    </button>
                  </div>
                ) : (
                  <div>
                    <h3 style={{
                      'text-decoration': todo.completed ? 'line-through' : 'none',
                      margin: '0 0 0.5rem 0'
                    }}>
                      {todo.title}
                    </h3>
                    {todo.description && (
                      <p style={{
                        'text-decoration': todo.completed ? 'line-through' : 'none',
                        margin: '0 0 0.5rem 0',
                        color: todo.completed ? '#888' : '#b8b8b8'
                      }}>
                        {todo.description}
                      </p>
                    )}
                    <div style={{ 'font-size': '0.8rem', color: '#888', 'margin-bottom': '0.5rem' }}>
                      Created: {new Date(todo.created_at).toLocaleString()}
                    </div>
                    <div>
                      <button
                        onClick={() => toggleComplete(todo)}
                        style={{
                          'margin-right': '0.5rem',
                          'background-color': todo.completed ? '#2d5a2d' : '#4a4a4a',
                          border: '1px solid #555',
                          color: 'rgba(255, 255, 255, 0.87)'
                        }}
                      >
                        {todo.completed ? 'Mark Incomplete' : 'Mark Complete'}
                      </button>
                      <button
                        onClick={() => setEditingTodo(todo)}
                        style={{
                          'margin-right': '0.5rem',
                          'background-color': '#3a5998',
                          border: '1px solid #555',
                          color: 'rgba(255, 255, 255, 0.87)'
                        }}
                      >
                        Edit
                      </button>
                      <button
                        onClick={() => deleteTodo(todo.id)}
                        style={{
                          'background-color': '#8b2635',
                          border: '1px solid #555',
                          color: 'rgba(255, 255, 255, 0.87)'
                        }}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}</For>
            <button onClick={fetchTodos} style={{ 'margin-top': '1rem' }}>
              Refresh TODOs
            </button>
          </div>
        ) : (
          <p>No todos yet. Add one above!</p>
        )}
      </div>

      <p class="read-the-docs">
        Click on the Vite and SolidJS logos to learn more
      </p>
    </>
  )
}

export default App
