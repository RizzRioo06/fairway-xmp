import { useState } from 'react';

export default function App() {
  const [count, setCount] = useState(0);
  return <main><h1>__PROJECT_NAME__</h1><p>React + Vite, created locally with Fairway.</p><button onClick={() => setCount(value => value + 1)}>Count: {count}</button></main>;
}
