[[ -z "$1" ]] && { echo "Please specify an 'id'" ; exit 1; }

id=$1


ctr=1
cd target
mkdir -p test_output
cd test_output

touch $id
echo "--------" >> $id

    while [ $ctr -le 5 ]
    do
        echo $ctr >> $id
        sleep 1
        ctr=$((ctr+1))
    done




    # x=1
# while [ $x -le 5 ]
# do
  # echo "Welcome $x times"
  # x=$(( $x + 1 ))
# done
