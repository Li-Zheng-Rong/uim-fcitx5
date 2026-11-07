;;; Fcitx5 engine for the UIM terminal frontend.
;;; Presentation uses UIM's existing preedit, candidate and mode primitives.

(require-dynlib "fcitx5")

(define-record 'fcitx5-context
  (append context-rec-spec
          '((handle #f)
            (fd #f)
            (watch-flags #f)
            (client-preedit ())
            (panel-preedit ())
            (aux-up "")
            (aux-down "")
            (rendered-preedit #f)
            (candidates ())
            (candidate-index -1)
            (showing-candidates #f)
            (mode-name "Fcitx5")
            (mode-id ""))))

(define fcitx5-notice
  (lambda (message)
    (uim-notify-info (string-append "uim-fcitx5: " message))))

(define fcitx5-disconnect!
  (lambda (c)
    (if (fcitx5-context-fd c)
        (uim-fep-unwatch-fd (fcitx5-context-fd c)))
    (fcitx5-context-set-fd! c #f)
    (fcitx5-context-set-watch-flags! c #f)
    (if (fcitx5-context-handle c)
        (fcitx5-native-abandon (fcitx5-context-handle c)))
    (fcitx5-context-set-handle! c #f)))

(define fcitx5-sync-watch!
  (lambda (c)
    (if (fcitx5-context-handle c)
        (let ((flags (fcitx5-native-watch-flags (fcitx5-context-handle c))))
          (if (not (equal? flags (fcitx5-context-watch-flags c)))
              (begin
                (uim-fep-update-fd (fcitx5-context-fd c) flags)
                (fcitx5-context-set-watch-flags! c flags)))))))

(define fcitx5-clear!
  (lambda (c)
    (fcitx5-context-set-client-preedit! c ())
    (fcitx5-context-set-panel-preedit! c ())
    (fcitx5-context-set-aux-up! c "")
    (fcitx5-context-set-aux-down! c "")
    (fcitx5-context-set-candidates! c ())
    (fcitx5-context-set-candidate-index! c -1)
    (if (fcitx5-context-showing-candidates c)
        (im-deactivate-candidate-selector c))
    (fcitx5-context-set-showing-candidates! c #f)))

(define fcitx5-render-preedit!
  (lambda (c)
    (let* ((panel (fcitx5-context-panel-preedit c))
           (main (if (any (lambda (segment) (not (string=? (cdr segment) ""))) panel)
                     panel
                     (fcitx5-context-client-preedit c)))
           (aux (string-append (fcitx5-context-aux-up c)
                               (fcitx5-context-aux-down c)))
           (segments (if (string=? aux "") main
                         (append main (list (cons preedit-none (string-append " " aux)))))))
      (if (not (equal? segments (fcitx5-context-rendered-preedit c)))
          (begin
            (context-update-preedit c segments)
            (fcitx5-context-set-rendered-preedit! c segments))))))

(define fcitx5-update-candidates!
  (lambda (c candidates index)
    (let ((page-changed? (not (equal? candidates (fcitx5-context-candidates c))))
          (index-changed? (not (= index (fcitx5-context-candidate-index c)))))
      (if page-changed?
          (begin
            (if (fcitx5-context-showing-candidates c)
                (im-deactivate-candidate-selector c))
            (fcitx5-context-set-showing-candidates! c #f)
            (fcitx5-context-set-candidates! c candidates)
            (if (pair? candidates)
                (let ((count (length candidates)))
                  ;; Fcitx sends the current page only. Do not invent a total.
                  (im-activate-candidate-selector c count count)
                  (fcitx5-context-set-showing-candidates! c #t)))))
      (if index-changed? (fcitx5-context-set-candidate-index! c index))
      (if (and (or page-changed? index-changed?)
               (fcitx5-context-showing-candidates c)
               (>= index 0) (< index (length candidates)))
          (im-select-candidate c index)))))

(define fcitx5-apply-events!
  (lambda (c events)
    (let ((preedit-changed? #f)
          (mode-changed? #f))
      (for-each
       (lambda (event)
         (case (car event)
           ((commit)
            (im-commit c (cadr event))
            ;; A following equal preedit still belongs after this commit.
            (fcitx5-context-set-rendered-preedit! c #f))
           ((preedit)
            (if (not (equal? (cadr event) (fcitx5-context-client-preedit c)))
                (begin
                  (fcitx5-context-set-client-preedit! c (cadr event))
                  (set! preedit-changed? #t)))
            (if (not (fcitx5-context-rendered-preedit c))
                (set! preedit-changed? #t)))
           ((panel)
            (let ((preedit (list-ref event 1))
                  (aux-up (list-ref event 2))
                  (aux-down (list-ref event 3)))
              (if (not (equal? preedit (fcitx5-context-panel-preedit c)))
                  (begin
                    (fcitx5-context-set-panel-preedit! c preedit)
                    (set! preedit-changed? #t)))
              (if (not (string=? aux-up (fcitx5-context-aux-up c)))
                  (begin
                    (fcitx5-context-set-aux-up! c aux-up)
                    (set! preedit-changed? #t)))
              (if (not (string=? aux-down (fcitx5-context-aux-down c)))
                  (begin
                    (fcitx5-context-set-aux-down! c aux-down)
                    (set! preedit-changed? #t))))
            (fcitx5-update-candidates! c (list-ref event 4) (list-ref event 5))
            (if (not (fcitx5-context-rendered-preedit c))
                (set! preedit-changed? #t)))
           ((mode)
            (if (not (string=? (cadr event) (fcitx5-context-mode-name c)))
                (begin
                  (fcitx5-context-set-mode-name! c (cadr event))
                  (set! mode-changed? #t)))
            (if (not (string=? (caddr event) (fcitx5-context-mode-id c)))
                (fcitx5-context-set-mode-id! c (caddr event))))
           ((focus-out)
            (fcitx5-clear! c)
            (set! preedit-changed? #t))
           ((notice) (fcitx5-notice (cadr event)))
           ((error)
            ;; A disconnected key is never replayed. A later, new key may
            ;; establish a fresh context after the original failure is shown.
            (fcitx5-disconnect! c)
            (fcitx5-clear! c)
            (if (not (string=? (fcitx5-context-mode-name c) "Fcitx5 unavailable"))
                (begin
                  (fcitx5-context-set-mode-name! c "Fcitx5 unavailable")
                  (set! mode-changed? #t)))
            (fcitx5-context-set-mode-id! c "")
            (fcitx5-notice (cadr event))
            (set! preedit-changed? #t))))
       events)
      (if preedit-changed? (fcitx5-render-preedit! c))
      (if mode-changed? (context-update-widgets c))
      (fcitx5-sync-watch! c))))

(define fcitx5-connect!
  (lambda (c)
    (or (fcitx5-context-handle c)
        (if (not (and (symbol-bound? 'uim-fep-watch-fd)
                      (symbol-bound? 'uim-fep-update-fd)))
            (begin
              (fcitx5-notice "This engine requires uim-fep with the descriptor-watch patch.")
              #f)
            (let ((handle (fcitx5-native-open)))
              (if (string? handle)
                  (begin (fcitx5-notice handle) #f)
                  (let ((fd (fcitx5-native-fd handle))
                        (flags (fcitx5-native-watch-flags handle)))
                    (if (uim-fep-watch-fd
                         fd flags (lambda ()
                              (if (fcitx5-context-handle c)
                                  (fcitx5-apply-events!
                                   c (fcitx5-native-dispatch (fcitx5-context-handle c))))))
                        (begin
                          (fcitx5-context-set-handle! c handle)
                          (fcitx5-context-set-fd! c fd)
                          (fcitx5-context-set-watch-flags! c flags)
                          handle)
                        (begin
                          (fcitx5-native-abandon handle)
                          (fcitx5-notice "The D-Bus descriptor cannot be watched by this frontend.")
                          #f)))))))))

(define fcitx5-init-handler
  (lambda (id im arg)
    (let ((c (fcitx5-context-new id im)))
      (fcitx5-context-set-widgets! c '(widget_fcitx5_input_mode))
      c)))

(define fcitx5-release-handler
  (lambda (c)
    (let ((handle (fcitx5-context-handle c)))
      (if (fcitx5-context-fd c)
          (uim-fep-unwatch-fd (fcitx5-context-fd c)))
      (fcitx5-context-set-fd! c #f)
      (fcitx5-context-set-watch-flags! c #f)
      (fcitx5-context-set-handle! c #f)
      (if handle (fcitx5-apply-events! c (fcitx5-native-close handle))))))

(define fcitx5-key-handler
  (lambda (c key state release?)
    (let ((handle (if release? (fcitx5-context-handle c) (fcitx5-connect! c))))
      (if handle
          (let ((result (fcitx5-native-key handle
                                          (if (symbol? key) (symbol->string key) key)
                                          state release?)))
            (fcitx5-apply-events! c (cdr result))
            (if (not (car result)) (im-commit-raw c)))
          (im-commit-raw c)))))

(define fcitx5-key-press-handler
  (lambda (c key state) (fcitx5-key-handler c key state #f)))
(define fcitx5-key-release-handler
  (lambda (c key state) (fcitx5-key-handler c key state #t)))

(define fcitx5-reset-handler
  (lambda (c)
    (if (fcitx5-context-handle c)
        (fcitx5-apply-events! c (fcitx5-native-reset (fcitx5-context-handle c))))))
(define fcitx5-focus-in-handler
  (lambda (c)
    (if (fcitx5-connect! c)
        (fcitx5-apply-events! c (fcitx5-native-focus-in (fcitx5-context-handle c))))))
(define fcitx5-focus-out-handler
  (lambda (c)
    (if (fcitx5-context-handle c)
        (fcitx5-apply-events! c (fcitx5-native-focus-out (fcitx5-context-handle c))))))
(define fcitx5-toggle-handler
  (lambda (c)
    (if (fcitx5-connect! c)
        (fcitx5-apply-events! c (fcitx5-native-toggle (fcitx5-context-handle c))))))

(define fcitx5-get-candidate-handler
  (lambda (c index accel-enum-hint)
    (list-ref (fcitx5-context-candidates c) index)))
(define fcitx5-select-candidate-handler
  (lambda (c index)
    (if (fcitx5-context-handle c)
        (fcitx5-apply-events! c (fcitx5-native-select-candidate
                               (fcitx5-context-handle c) index)))))

(register-action
 'action_fcitx5_mode
 (lambda (c)
   (let ((name (fcitx5-context-mode-name c)))
     (list 'fcitx5 name name "Fcitx5 input mode (Win+Space)")))
 (lambda (c) #t)
 fcitx5-toggle-handler)
(register-widget 'widget_fcitx5_input_mode
                 (activity-indicator-new '(action_fcitx5_mode))
                 (actions-new '(action_fcitx5_mode)))

(register-im
 'fcitx5 "*" "UTF-8" "Fcitx5" "Fcitx5 input method through UIM" #f
 fcitx5-init-handler fcitx5-release-handler context-mode-handler
 fcitx5-key-press-handler fcitx5-key-release-handler fcitx5-reset-handler
 fcitx5-get-candidate-handler fcitx5-select-candidate-handler
 context-prop-activate-handler #f
 fcitx5-focus-in-handler fcitx5-focus-out-handler #f fcitx5-reset-handler)
