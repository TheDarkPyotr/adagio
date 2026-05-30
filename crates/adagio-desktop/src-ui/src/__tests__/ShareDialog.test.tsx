import { render, screen, fireEvent } from '@testing-library/react';
import ShareDialog from '../components/ShareDialog';

describe('ShareDialog', () => {
  const onClose = vi.fn();
  beforeEach(() => vi.clearAllMocks());

  it('renders the filename extracted from path', () => {
    render(<ShareDialog path="/docs/report.pdf" onClose={onClose} />);
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
  });

  it('renders the full path', () => {
    render(<ShareDialog path="/docs/report.pdf" onClose={onClose} />);
    expect(screen.getByText('/docs/report.pdf')).toBeInTheDocument();
  });

  it('calls onClose when backdrop is clicked', () => {
    const { container } = render(<ShareDialog path="/file.txt" onClose={onClose} />);
    fireEvent.click(container.firstChild!);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not call onClose when the inner dialog panel is clicked', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    fireEvent.click(screen.getByText('file.txt'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('calls onClose when the X close button is clicked', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    // X button is first button in the dialog
    fireEvent.click(screen.getAllByRole('button')[0]);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('adds a recipient chip when Enter is pressed in the input', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    const input = screen.getByPlaceholderText('Add a name or email…');
    fireEvent.change(input, { target: { value: 'alice@example.com' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(screen.getByText('alice@example.com')).toBeInTheDocument();
  });

  it('adds a recipient chip when comma is pressed in the input', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    const input = screen.getByPlaceholderText('Add a name or email…');
    fireEvent.change(input, { target: { value: 'bob@example.com' } });
    fireEvent.keyDown(input, { key: ',' });
    expect(screen.getByText('bob@example.com')).toBeInTheDocument();
  });

  it('clears the input after adding a chip', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    const input = screen.getByPlaceholderText('Add a name or email…') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'carol@example.com' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(input.value).toBe('');
  });

  it('renders the permission segmented control with View, Comment, Edit options', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    expect(screen.getByText('View')).toBeInTheDocument();
    expect(screen.getByText('Comment')).toBeInTheDocument();
    expect(screen.getByText('Edit')).toBeInTheDocument();
  });

  it('renders the expiry segmented control with time options', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    expect(screen.getByText('7 days')).toBeInTheDocument();
    expect(screen.getByText('30 days')).toBeInTheDocument();
    expect(screen.getByText('No limit')).toBeInTheDocument();
  });

  it('renders the copy button in the link bar', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    expect(screen.getByText('copy')).toBeInTheDocument();
  });

  it('shows "copied" after the copy button is clicked', () => {
    render(<ShareDialog path="/file.txt" onClose={onClose} />);
    fireEvent.click(screen.getByText('copy'));
    expect(screen.getByText('copied')).toBeInTheDocument();
  });
});
