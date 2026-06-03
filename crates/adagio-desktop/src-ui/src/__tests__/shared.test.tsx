import { render, screen } from '@testing-library/react';
import { Icon, SpinDot, StatusDot, FileGlyph } from '../components/shared';

describe('Icon', () => {
  it('renders an svg for every known icon name', () => {
    const names = ['folder', 'file', 'search', 'plus', 'check', 'settings', 'sync', 'share', 'trash', 'refresh'] as const;
    for (const name of names) {
      const { container } = render(<Icon name={name} />);
      expect(container.querySelector('svg')).not.toBeNull();
    }
  });

  it('uses the supplied size', () => {
    const { container } = render(<Icon name="folder" size={32} />);
    const svg = container.querySelector('svg')!;
    expect(svg.getAttribute('width')).toBe('32');
    expect(svg.getAttribute('height')).toBe('32');
  });
});

describe('SpinDot', () => {
  it('renders', () => {
    const { container } = render(<SpinDot />);
    expect(container.querySelector('svg')).not.toBeNull();
  });
});

describe('StatusDot', () => {
  it.each(['ok', 'sync', 'cloud', 'pin', 'conflict'] as const)('renders for status %s', (status) => {
    const { container } = render(<StatusDot status={status} />);
    expect(container.firstChild).not.toBeNull();
  });
});

describe('FileGlyph', () => {
  it('renders for a folder', () => {
    const { container } = render(<FileGlyph kind="folder" />);
    expect(container.firstChild).not.toBeNull();
  });

  it('renders for an unknown extension', () => {
    const { container } = render(<FileGlyph kind="xyz" />);
    expect(container.firstChild).not.toBeNull();
  });
});
