import { Component, type ReactNode } from 'react';

// Keep failures local so a broken view cannot tear down a healthy terminal.
// A reload is always explicit: replacing the renderer closes embedded shells.
export default class RecoveryBoundary extends Component<{ name: string; children: ReactNode; reload?: boolean; onError?: () => void }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch() { this.props.onError?.(); }

  render() {
    if (!this.state.failed) return this.props.children;
    return <section className="settings-card interface-recovery" aria-label={`${this.props.name} recovery`}>
      <h2>{this.props.name} could not be displayed</h2>
      <p role="alert">Your project files and databases are kept. Services continue running.</p>
      {this.props.reload ? <>
        <p>Reload the interface to try again. Reloading closes embedded terminal sessions.</p>
        <button className="primary" onClick={() => window.location.reload()}>Reload interface</button>
      </> : <>
        <p>Other views remain available. Retry this view when you are ready.</p>
        <button className="primary" onClick={() => this.setState({ failed: false })}>Retry view</button>
      </>}
    </section>;
  }
}
