package dev.blitz.shell;

import android.app.Activity;
import android.content.Context;
import android.text.Editable;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

/** Public Android editor connection; the renderer and authoritative editor stay in Rust. */
public final class TextInputHost {
    private final Activity activity;
    private final long handle;
    private volatile boolean closed;
    private ViewGroup decor;
    private EditorView view;
    private View previousFocus;
    private Editable text = new SpannableStringBuilder();
    private long session;
    private long sequence;
    private boolean active;
    private int inputType;
    private int imeOptions;
    private int batchDepth;
    private boolean dirty;
    private String publishedText;
    private int publishedStart, publishedEnd, publishedComposeStart, publishedComposeEnd;

    private static native void onState(long handle, long session, long sequence,
            String text, int start, int end, int composeStart, int composeEnd);
    private static native void onAction(long handle, long session, long sequence, int action);

    private TextInputHost(Activity activity, long handle) {
        this.activity = activity;
        this.handle = handle;
    }
    public static TextInputHost attach(final Activity activity, final long handle) {
        final TextInputHost host = new TextInputHost(activity, handle);
        activity.runOnUiThread(new Runnable() {
            @Override public void run() {
                if (host.closed) return;
                host.decor = (ViewGroup) activity.getWindow().getDecorView();
                host.view = host.new EditorView();
                // It only owns editor focus. A transparent non-clickable 1px View
                // never covers or substitutes the native canvas's touch targets.
                host.decor.addView(host.view, new ViewGroup.LayoutParams(1, 1));
            }
        });
        return host;
    }
    private InputMethodManager manager() {
        return (InputMethodManager) activity.getSystemService(Context.INPUT_METHOD_SERVICE);
    }
    private void restore(String value, int start, int end, int composeStart, int composeEnd) {
        text.replace(0, text.length(), value);
        BaseInputConnection.removeComposingSpans(text);
        Selection.setSelection(text, Math.max(0, Math.min(start, text.length())),
                Math.max(0, Math.min(end, text.length())));
        if (composeStart >= 0 && composeEnd > composeStart && composeEnd <= text.length()) {
            // The standard connection installs Android's composing spans.
            new Connection(session).setRegionWithoutPublishing(composeStart, composeEnd);
        }
        remember();
    }
    private void remember() {
        publishedText = text.toString();
        publishedStart = Selection.getSelectionStart(text);
        publishedEnd = Selection.getSelectionEnd(text);
        publishedComposeStart = BaseInputConnection.getComposingSpanStart(text);
        publishedComposeEnd = BaseInputConnection.getComposingSpanEnd(text);
    }
    /** Never overwrite a newer UI-thread edit that Rust has not acknowledged. */
    public void update(final long editorSession, final long acknowledged, final String value,
            final int start, final int end, final int composeStart, final int composeEnd,
            final int type, final int options) {
        activity.runOnUiThread(new Runnable() {
            @Override public void run() {
                if (closed || decor == null) return;
                boolean newSession = session != editorSession;
                if (!newSession && (sequence > acknowledged || (batchDepth > 0 && dirty))) return;
                boolean optionsChanged = inputType != type || imeOptions != options;
                inputType = type;
                imeOptions = options;
                if (newSession) {
                    session = editorSession;
                    sequence = 0;
                    batchDepth = 0;
                    dirty = false;
                    text = new SpannableStringBuilder();
                }
                boolean wasActive = active;
                active = editorSession != 0;
                restore(value, start, end, composeStart, composeEnd);
                if (!active) {
                    if (wasActive) {
                        manager().hideSoftInputFromWindow(view.getWindowToken(), 0);
                        view.clearFocus();
                        if (previousFocus != null) previousFocus.requestFocus();
                        previousFocus = null;
                    }
                    return;
                }
                if (!wasActive || newSession || optionsChanged) {
                    if (!wasActive) previousFocus = decor.findFocus();
                    view.requestFocus();
                    manager().restartInput(view);
                    view.post(new Runnable() {
                        @Override public void run() {
                            if (!closed && active && session == editorSession) {
                                manager().showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
                            }
                        }
                    });
                }
                manager().updateSelection(view, Selection.getSelectionStart(text),
                        Selection.getSelectionEnd(text), BaseInputConnection.getComposingSpanStart(text),
                        BaseInputConnection.getComposingSpanEnd(text));
            }
        });
    }
    private final class EditorView extends View {
        EditorView() {
            super(activity);
            setWillNotDraw(true);
            setFocusable(true);
            setFocusableInTouchMode(true);
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        }
        @Override public boolean onCheckIsTextEditor() { return active && !closed; }
        @Override public InputConnection onCreateInputConnection(EditorInfo info) {
            if (!active || closed) return null;
            info.inputType = inputType;
            info.imeOptions = imeOptions;
            info.initialSelStart = Selection.getSelectionStart(text);
            info.initialSelEnd = Selection.getSelectionEnd(text);
            return new Connection(session);
        }
    }
    private void changed() {
        dirty = true;
        publish();
    }
    private void publish() {
        if (!active || closed || batchDepth > 0 || !dirty) return;
        dirty = false;
        String value = text.toString();
        int start = Selection.getSelectionStart(text), end = Selection.getSelectionEnd(text);
        int composeStart = BaseInputConnection.getComposingSpanStart(text);
        int composeEnd = BaseInputConnection.getComposingSpanEnd(text);
        if (value.equals(publishedText) && start == publishedStart && end == publishedEnd
                && composeStart == publishedComposeStart && composeEnd == publishedComposeEnd) return;
        remember();
        onState(handle, session, ++sequence, value, start, end, composeStart, composeEnd);
        manager().updateSelection(view, start, end, composeStart, composeEnd);
    }
    /** A fresh user activation should reopen a dismissed keyboard even if focus is unchanged. */
    public void show(final long editorSession) {
        activity.runOnUiThread(new Runnable() {
            @Override public void run() {
                if (!closed && active && session == editorSession && view != null) {
                    view.requestFocus();
                    manager().showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
                }
            }
        });
    }
    public void close() {
        closed = true;
        activity.runOnUiThread(new Runnable() {
            @Override public void run() {
                if (active && view != null) manager().hideSoftInputFromWindow(view.getWindowToken(), 0);
                active = false;
                text.clear();
                if (view != null) view.clearFocus();
                if (previousFocus != null) previousFocus.requestFocus();
                previousFocus = null;
                if (decor != null && view != null) decor.removeView(view);
                view = null;
                decor = null;
            }
        });
    }
    private final class Connection extends BaseInputConnection {
        private final long editorSession;
        private boolean connectionClosed;
        Connection(long editorSession) {
            super(view, true);
            this.editorSession = editorSession;
        }
        private boolean valid() { return !closed && !connectionClosed && active && session == editorSession; }
        @Override public Editable getEditable() {
            return valid() ? text : new SpannableStringBuilder();
        }
        void setRegionWithoutPublishing(int start, int end) { super.setComposingRegion(start, end); }
        @Override public boolean beginBatchEdit() {
            if (!valid()) return false;
            batchDepth++;
            return true;
        }
        @Override public boolean endBatchEdit() {
            if (!valid()) return false;
            if (batchDepth > 0) batchDepth--;
            publish();
            return batchDepth > 0;
        }
        @Override public boolean commitText(CharSequence value, int cursor) {
            if (!valid()) return false;
            boolean accepted = super.commitText(value, cursor); changed(); return accepted;
        }
        @Override public boolean setComposingText(CharSequence value, int cursor) {
            if (!valid()) return false;
            boolean accepted = super.setComposingText(value, cursor); changed(); return accepted;
        }
        @Override public boolean setComposingRegion(int start, int end) {
            if (!valid()) return false;
            boolean accepted = super.setComposingRegion(start, end); changed(); return accepted;
        }
        @Override public boolean finishComposingText() {
            if (!valid()) return false;
            boolean accepted = super.finishComposingText(); changed(); return accepted;
        }
        @Override public boolean setSelection(int start, int end) {
            if (!valid()) return false;
            boolean accepted = super.setSelection(start, end); changed(); return accepted;
        }
        @Override public boolean deleteSurroundingText(int before, int after) {
            if (!valid()) return false;
            boolean accepted = super.deleteSurroundingText(before, after); changed(); return accepted;
        }
        @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) {
            if (!valid()) return false;
            boolean accepted = super.deleteSurroundingTextInCodePoints(before, after); changed(); return accepted;
        }
        @Override public boolean sendKeyEvent(KeyEvent event) {
            return valid() && super.sendKeyEvent(event);
        }
        @Override public boolean performEditorAction(int action) {
            if (!valid()) return false;
            finishComposingText();
            publish();
            onAction(handle, session, sequence, action);
            return true;
        }
        @Override public void closeConnection() {
            if (valid()) {
                batchDepth = 0;
                super.closeConnection();
                publish();
            }
            connectionClosed = true;
        }
    }
}
