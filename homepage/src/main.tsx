import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@fontsource-variable/quicksand'
import './styles/index.css'
import App from './App'

const root = document.getElementById('root')
if (!root) throw new Error('Missing #root element')

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
