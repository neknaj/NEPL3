# Public candidate acquisition

Base 4325477d288e00bc9f4d1c7034e3c43dbaf19e94. acquire connects authenticated
run/jobs/main observations, two artifact metadata/ZIP acquisitions and the
production prepare_upload validators. It rechecks run/jobs/main at completion.
The caller supplies independent payload pins and still owns writer lock,
permission, current-publication and recovery gates. This performs no mutation.

Root and independent normal/-O suites each passed 5 acquisition tests, using
real payload/tar validators with only network acquisition substituted. The
review also checked decreasing shared-budget allowances and late-main rejection.
Repository check passed. Success retains raw evidence; failure raises without
partial evidence. No live end-to-end acquisition or publication is claimed.
The full site suite is running separately and is not recorded as passed here.
