package dev.blitz.shell;

import android.app.Activity;
import android.os.Build;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewTreeObserver;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import java.util.Arrays;
import java.util.List;

/** Observes the winit-owned Activity window. All View access stays on the UI thread. */
public final class InsetsObserver implements ViewTreeObserver.OnGlobalLayoutListener,
        View.OnApplyWindowInsetsListener {
    private final Activity activity;
    private final long handle;
    private volatile boolean closed;
    private View view;
    private ViewGroup decor;
    private int[] previous;

    private static native void onInsets(long handle, int left, int top, int right, int bottom);

    private InsetsObserver(Activity activity, long handle) {
        this.activity = activity;
        this.handle = handle;
    }
    public static InsetsObserver attach(Activity activity, long handle) {
        final InsetsObserver observer = new InsetsObserver(activity, handle);
        activity.runOnUiThread(new Runnable() {
            @Override public void run() { observer.install(); }
        });
        return observer;
    }
    private void install() {
        if (closed) return;
        // Own a transparent observer child, so attaching/detaching never replaces
        // another library's listeners on the Activity's decor or content views.
        decor = (ViewGroup) activity.getWindow().getDecorView();
        view = new View(activity);
        // Keep it visible to receive inset-animation dispatch. A plain View with
        // no background, drawing, click or focus handlers remains transparent
        // and passes pointer input through to the native content beneath it.
        view.setWillNotDraw(true);
        view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        view.setOnApplyWindowInsetsListener(this);
        if (Build.VERSION.SDK_INT >= 30) Api30.installAnimation(view, this);
        decor.addView(view, new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
        view.getViewTreeObserver().addOnGlobalLayoutListener(this);
        publish(view.getRootWindowInsets());
        view.requestApplyInsets();
    }
    private void publish(WindowInsets insets) {
        if (closed || insets == null || view == null || view.getWidth() == 0 || view.getHeight() == 0) return;
        int[] values;
        if (Build.VERSION.SDK_INT >= 30) values = Api30.values(insets);
        else values = new int[] { insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom() };
        int[] state = { values[0], values[1], values[2], values[3], view.getWidth(), view.getHeight() };
        if (!Arrays.equals(previous, state)) {
            previous = state;
            onInsets(handle, values[0], values[1], values[2], values[3]);
        }
    }
    @Override public void onGlobalLayout() { publish(view.getRootWindowInsets()); }
    @Override public WindowInsets onApplyWindowInsets(View view, WindowInsets insets) {
        publish(insets);
        return view.onApplyWindowInsets(insets);
    }
    public void close() {
        closed = true;
        activity.runOnUiThread(new Runnable() {
            @Override public void run() {
                if (view == null) return;
                ViewTreeObserver tree = view.getViewTreeObserver();
                if (tree.isAlive()) {
                    tree.removeOnGlobalLayoutListener(InsetsObserver.this);
                }
                view.setOnApplyWindowInsetsListener(null);
                if (Build.VERSION.SDK_INT >= 30) Api30.removeAnimation(view);
                decor.removeView(view);
                view = null;
                decor = null;
            }
        });
    }
    // Keep API30 classes in an outline so API26-29 do not resolve them on load.
    private static final class Api30 {
        static int[] values(WindowInsets insets) {
            android.graphics.Insets value = insets.getInsets(WindowInsets.Type.systemBars()
                    | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
            return new int[] { value.left, value.top, value.right, value.bottom };
        }
        static void installAnimation(View view, final InsetsObserver observer) {
            view.setWindowInsetsAnimationCallback(new WindowInsetsAnimation.Callback(
                    WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
                @Override public WindowInsets onProgress(WindowInsets insets, List<WindowInsetsAnimation> running) {
                    observer.publish(insets);
                    return insets;
                }
                @Override public void onEnd(WindowInsetsAnimation animation) {
                    if (!observer.closed && observer.view != null) observer.publish(observer.view.getRootWindowInsets());
                }
            });
        }
        static void removeAnimation(View view) { view.setWindowInsetsAnimationCallback(null); }
    }
}
