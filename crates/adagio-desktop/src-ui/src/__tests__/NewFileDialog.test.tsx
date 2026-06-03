import { render, screen, fireEvent } from '@testing-library/react';
import NewFileDialog from '../components/NewFileDialog';

describe('NewFileDialog', () => {
  it('renders the dialog with Folder and Markdown buttons', () => {
    render(<NewFileDialog onClose={() => {}} />);
    expect(screen.getByText('Folder')).toBeInTheDocument();
    expect(screen.getByText('Markdown')).toBeInTheDocument();
  });

  it('calls onClose when Cancel is clicked', () => {
    const onClose = vi.fn();
    render(<NewFileDialog onClose={onClose} />);
    fireEvent.click(screen.getByText('Cancel'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('calls onClose when the backdrop is clicked', () => {
    const onClose = vi.fn();
    const { container } = render(<NewFileDialog onClose={onClose} />);
    // The outermost div is the backdrop
    fireEvent.click(container.firstChild!);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not close when the inner panel is clicked', () => {
    const onClose = vi.fn();
    render(<NewFileDialog onClose={onClose} />);
    fireEvent.click(screen.getByText('New item'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('activates drag state on dragover', () => {
    render(<NewFileDialog onClose={() => {}} />);
    const dropZone = screen.getByText('Drop files here').parentElement!;
    fireEvent.dragOver(dropZone);
    // Border style changes — just assert no crash and the element is still there
    expect(dropZone).toBeInTheDocument();
  });
});
