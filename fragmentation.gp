# Plot the output of `cargo run --release --bin fragmentation`.
#
# Usage: gnuplot fragmentation.gp
#    or: gnuplot -e "data='other.dat'" fragmentation.gp
# To save the plot in a png instead of opening a window:
#        gnuplot -e "output='fragmentation.png'" fragmentation.gp

if (!exists("data")) data = 'fragmentation.dat'

# the first column is the cycle, then there are three columns per engine
stats data skip 1 nooutput
engines = (STATS_columns - 1) / 3

set key outside right
set xlabel 'cycle'
set grid

# the cycle 0 is the initial indexing, there is nothing to delete
deletion = "set title 'Deletion time'; set ylabel 'ms'; unset logscale y; \
    plot for [i=0:engines-1] data every ::1 using 1:(column(2 + 3*i)) title columnhead(2 + 3*i) with lines"
addition = "set title 'Addition time (cycle 0 is the initial indexing)'; set ylabel 'ms'; set logscale y; \
    plot for [i=0:engines-1] data using 1:(column(3 + 3*i)) title columnhead(3 + 3*i) with lines"
size = "set title 'Database size on disk'; set ylabel 'MB'; unset logscale y; \
    plot for [i=0:engines-1] data using 1:(column(4 + 3*i)) title columnhead(4 + 3*i) with lines"

if (exists("output")) {
    set terminal pngcairo size 1200,1400 noenhanced
    set output output
    set multiplot layout 3,1
    eval deletion
    eval addition
    eval size
    unset multiplot
} else {
    # one window per chart, gnuplot can't zoom or move a multiplot
    set termoption noenhanced
    set terminal qt 0 title 'Deletion time'
    eval deletion
    set terminal qt 1 title 'Addition time'
    eval addition
    set terminal qt 2 title 'Database size'
    eval size
    # keep the windows open, closing the database size window exits
    pause mouse close
}
